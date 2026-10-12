//! Explicit reading through native graphs, retaining the existing product result
//! shape. Detached work settles even when an IPC consumer disappears.
use super::*;
use crate::ai::workspace_graph::error;
use crate::model::Result;
use crate::{
    ai::{graph::*, results::Retained, transport::graph_identity, workspace_graph::Progress},
    language::{
        gloss_graph,
        reading::{self, ReadingAid, Request},
        translation_graph,
    },
};

impl Application {
    pub(super) async fn native_reading(
        self: &Arc<Self>,
        request: Arc<Request>,
    ) -> Result<Retained> {
        let state = self.clone();
        let worker_request = request.clone();
        let (sender, mut receiver) = tokio::sync::oneshot::channel();
        tokio::spawn(async move {
            let mut result = execute(&state, &worker_request, || !sender.is_closed()).await;
            if let Err(failure) = &mut result {
                let cancellation = (|| -> Result<()> {
                    let mut guard = state.lock()?;
                    if guard.learner_id()? == worker_request.install {
                        let store: &mut Store = &mut guard;
                        store
                            .workspace_graphs
                            .cancel_run(&mut store.connection, &worker_request.id)
                            .map_err(error)?;
                    }
                    Ok(())
                })();
                if let Err(cause) = cancellation {
                    failure.diagnostics = Some(
                        serde_json::json!({"original":failure.diagnostics,"cancellation":crate::diagnostics::response::error_metadata(&cause, &[])}),
                    );
                }
            }
            let _ = sender.send(result);
        });
        loop {
            tokio::select! {
                result = &mut receiver => return result.map_err(|_| AppError::new(ErrorCode::UnknownOutcome, "Reading execution ended without reporting its outcome."))?,
                _ = tokio::time::sleep(Duration::from_millis(10)) => request.validate_source(&*self.lock()?)?,
            }
        }
    }
}
async fn execute(
    state: &Arc<Application>,
    request: &Request,
    consumer_alive: impl Fn() -> bool,
) -> Result<Retained> {
    let weak = Arc::downgrade(state);
    let graph = match request.input.aid {
        ReadingAid::Translation => translation_graph::compile(Arc::new(move |invocation, _| {
            let state = weak.upgrade();
            Box::pin(async move {
                super::workspace_text::provide(state.ok_or_else(unavailable)?, invocation).await
            })
        })),
        ReadingAid::Explanations | ReadingAid::Completions => {
            reading::explanation_graph::compile(Arc::new(move |invocation, _| {
                let state = weak.upgrade();
                Box::pin(async move {
                    super::workspace_text::provide(state.ok_or_else(unavailable)?, invocation).await
                })
            }))
        }
        ReadingAid::WordGloss => gloss_graph::compile(Arc::new(move |invocation, _| {
            let state = weak.upgrade();
            Box::pin(async move {
                super::workspace_text::provide(state.ok_or_else(unavailable)?, invocation).await
            })
        })),
        _ => {
            return Err(AppError::new(
                ErrorCode::Validation,
                "Unsupported native reading aid.",
            ));
        }
    }
    .map_err(error)?;
    let node = match request.input.aid {
        ReadingAid::Translation => "translate",
        ReadingAid::Explanations | ReadingAid::Completions => "operation",
        _ => "gloss",
    };
    let mut context = request.native_context()?;
    let catalog = {
        let mut guard = state.lock()?;
        request.validate_source(&guard)?;
        let reuse_scope = reading::native_cache::scope(
            &guard.connection,
            context["sourceKey"].as_str().unwrap(),
            request.fresh,
            &request.id,
        )?;
        context["reuseScope"] = serde_json::json!(reuse_scope);
        let store: &mut Store = &mut guard;
        let catalog = store
            .workspace_graphs
            .begin(
                &mut store.connection,
                &request.install,
                Arc::new(graph),
                &request.id,
                request.native_inputs()?,
                &reuse_scope,
                request.input.aid.receipt_kind(),
                &context,
                |_, _| Ok(()),
            )
            .map_err(error)?;
        if matches!(
            request.input.aid,
            ReadingAid::Translation | ReadingAid::WordGloss
        ) {
            store
                .workspace_graphs
                .event(
                    &mut store.connection,
                    &catalog,
                    Event::Demand {
                        run: request.id.clone(),
                        node: node.into(),
                    },
                    |_, _| Ok(()),
                )
                .map_err(error)?;
        }
        catalog
    };
    loop {
        let progress = {
            let mut guard = state.lock()?;
            let authority = request.validate_source(&guard);
            let paused = crate::ai::connections::configuration::config(&guard.connection)?.paused;
            let store: &mut Store = &mut guard;
            if authority.is_err() || !consumer_alive() {
                store
                    .workspace_graphs
                    .event(
                        &mut store.connection,
                        &catalog,
                        Event::Cancel {
                            run: request.id.clone(),
                            node: None,
                        },
                        |_, _| Ok(()),
                    )
                    .map_err(error)?;
                return Err(authority.err().unwrap_or_else(|| {
                    AppError::new(ErrorCode::Conflict, "Reading consumer closed.")
                }));
            }
            store
                .workspace_graphs
                .poll(
                    &mut store.connection,
                    &catalog,
                    &request.id,
                    Capacity {
                        local: usize::MAX,
                        provider: if paused {
                            0
                        } else {
                            crate::ai::policy::admission::NETWORK_CAPACITY
                        },
                    },
                    |db, event| {
                        if matches!(event.intent, CommitIntent::Dispatch { .. }) {
                            graph_identity::bind_workspace(db, event).map_err(|_| unavailable())?;
                        }
                        Ok(())
                    },
                )
                .map_err(error)?
        };
        match progress {
            Progress::Held => {
                return Err(AppError::new(
                    ErrorCode::AdmissionHeld,
                    "AI execution is paused.",
                ));
            }
            Progress::Waiting => tokio::time::sleep(Duration::from_millis(10)).await,
            Progress::Invoke(invocation) => {
                let report = invocation
                    .execute(EvidenceLimits {
                        observations: 64,
                        bytes: 1024 * 1024,
                    })
                    .await;
                let mut guard = state.lock()?;
                if guard.learner_id()? != request.install {
                    return Err(AppError::new(
                        ErrorCode::SessionExpired,
                        "Workspace changed during reading.",
                    ));
                }
                let settled = report.outcome.clone();
                let store: &mut Store = &mut guard;
                store.workspace_graphs.event(&mut store.connection, &catalog, Event::Observe(EvidenceSnapshot {
                    identity: report.identity.clone(), observations: report.observations.clone(), evidence_failure: report.evidence_failure.clone(), provisional: report.provisional.clone(),
                }), |_, _| Ok(())).map_err(|fault| error(fault).with_diagnostics(serde_json::json!({"response":{"observations":report.observations},"nativeSettlement":"uncommitted"})))?;
                if let Err(fault) = store.workspace_graphs.event(
                    &mut store.connection,
                    &catalog,
                    Event::SettleObserved(report),
                    |_, _| Ok(()),
                ) {
                    let mut failure = error(fault);
                    let receipt =
                        crate::ai::workspace_graph::receipt(&store.connection, &request.id)?;
                    failure.diagnostics = Some(
                        serde_json::json!({"storage":failure.diagnostics,"response":receipt.map(|r| r["response"].clone())}),
                    );
                    return Err(failure);
                }
                if let Ok(values) = settled {
                    request.validate_access(store)?;
                    let (engine, attempt, _) = store
                        .workspace_graphs
                        .result_record(&store.connection, &catalog, &request.id, node)
                        .map_err(error)?;
                    let mut stored = reading::native::project(
                        &values,
                        &context,
                        &engine,
                        &request.id,
                        node,
                        attempt.id,
                    )?;
                    if let Some(new) = stored.gloss.take() {
                        stored.gloss = Some(
                            if let Some(old) = reading::native_cache::previous(
                                &store.connection,
                                context["sourceKey"].as_str().unwrap(),
                            )?
                            .and_then(|s| s.gloss)
                            {
                                crate::language::gloss::merge_repair(&old, new)?
                            } else {
                                new
                            },
                        );
                    }
                    reading::native_cache::retain(
                        &store.connection,
                        &request.id,
                        context["sourceKey"].as_str().unwrap(),
                        &stored,
                    )
                    .map_err(|mut error| {
                        if let Ok(Some(receipt)) =
                            crate::ai::workspace_graph::receipt(&store.connection, &request.id)
                        {
                            error.diagnostics =
                                Some(serde_json::json!({"response":receipt["response"]}));
                        }
                        error
                    })?;
                }
            }
            Progress::Complete(values) => {
                let store = state.lock()?;
                request.validate_source(&store)?;
                let (engine, attempt, evidence) = store
                    .workspace_graphs
                    .result_record(&store.connection, &catalog, &request.id, node)
                    .map_err(error)?;
                let mut stored = reading::native::project(
                    &values,
                    &context,
                    &engine,
                    &request.id,
                    node,
                    attempt.id,
                )?;
                if let Some(new) = stored.gloss.take() {
                    stored.gloss = Some(
                        if let Some(old) = reading::native_cache::previous(
                            &store.connection,
                            context["sourceKey"].as_str().unwrap(),
                        )?
                        .and_then(|s| s.gloss)
                        {
                            crate::language::gloss::merge_repair(&old, new)?
                        } else {
                            new
                        },
                    );
                }
                reading::native_cache::retain(
                    &store.connection,
                    &request.id,
                    context["sourceKey"].as_str().unwrap(),
                    &stored,
                )?;
                let execution = format!(
                    "graph:{}",
                    serde_json::to_string(&(&engine, attempt.execution))?
                );
                let cached = attempt.acquisition != Acquisition::Produced;
                let mut metadata = serde_json::json!({"requestedModel":request.model,"route":request.target.route.label(),"nativeEngine":engine,"nativeRun":request.id,"nativeAttempt":attempt.id,"nativeExecution":attempt.execution,"sourceExecutionId":execution,"cacheHit":cached,"nativeEvidence":evidence});
                if let Some(evidence) = evidence {
                    for observation in evidence.observations {
                        if let Some(EvidenceValue::ClassifiedJson(value)) =
                            observation.additional.get("gloss_recovery")
                        {
                            metadata["wordGlossValidation"] = value.clone();
                        }
                        if let Some(value) = observation.actual_model {
                            metadata["actualModel"] = serde_json::json!(value);
                        }
                        if let Some(value) = observation.request_id {
                            metadata["providerId"] = serde_json::json!(value);
                        }
                        if let Some(value) = observation.finish_reason {
                            metadata["finishReason"] = serde_json::json!(value);
                        }
                        if let Some(usage) = observation.usage {
                            metadata["inputTokens"] = serde_json::json!(usage.input_tokens);
                            metadata["outputTokens"] = serde_json::json!(usage.output_tokens);
                        }
                    }
                }
                return Ok(Retained {
                    cached,
                    execution,
                    payload: serde_json::to_vec(&stored)?,
                    metadata,
                });
            }
            Progress::Failed(cause) => {
                let store = state.lock()?;
                let (engine, attempt, evidence) = store
                    .workspace_graphs
                    .result_record(&store.connection, &catalog, &request.id, node)
                    .map_err(error)?;
                let receipt = crate::ai::workspace_graph::receipt(&store.connection, &request.id)?;
                let domain = evidence
                    .as_ref()
                    .and_then(|e| {
                        e.observations.iter().rev().find_map(|o| {
                            let code = serde_json::from_value::<ErrorCode>(serde_json::json!(
                                o.error_code.as_ref()?
                            ))
                            .ok()?;
                            Some(AppError::new(
                                code,
                                o.redacted_reason
                                    .as_deref()
                                    .unwrap_or("Native reading failed."),
                            ))
                        })
                    })
                    .unwrap_or_else(|| error(cause));
                return Err(domain.with_diagnostics(serde_json::json!({"nativeEngine":engine,"nativeAttempt":attempt.id,"response":receipt.map(|r| r["response"].clone())})));
            }
            Progress::Unknown => {
                return Err(AppError::new(
                    ErrorCode::UnknownOutcome,
                    "Reading outcome is unknown. Request a retry explicitly.",
                ));
            }
            Progress::Cancelled => {
                return Err(AppError::new(
                    ErrorCode::Conflict,
                    "Reading request was cancelled.",
                ));
            }
        }
    }
}
fn unavailable() -> Fault {
    Fault {
        code: "workspace_reading_unavailable".into(),
        path: "owner".into(),
    }
}
