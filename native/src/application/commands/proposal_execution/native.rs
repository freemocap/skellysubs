//! Native proposal host. Candidate generation is fresh; review/acceptance is
//! still owned by the existing persona/Drill command transaction.
use super::*;
use crate::ai::workspace_graph::error;
use crate::ai::{
    generation::graph as proposal, graph::*, transport::graph_identity, workspace_graph::Progress,
};
use crate::model::Result;

pub(super) async fn produce<T>(
    state: &Arc<Application>,
    request: &Arc<generation::Request>,
    task: Task,
    parse: impl FnOnce(&Store, &generation::Request, &provider::Completion) -> Result<T>,
    consumer_alive: impl Fn() -> bool,
) -> Output<T> {
    let mut completion = None;
    let mut outcome = async {
        let kind = match request.kind {
            "persona" => proposal::Kind::Persona,
            "drill" => proposal::Kind::Drill,
            _ => {
                return Err(AppError::new(
                    ErrorCode::Validation,
                    "Unsupported proposal kind.",
                ));
            }
        };
        let weak = Arc::downgrade(state);
        let graph = proposal::compile(
            kind,
            Arc::new(move |invocation, _| {
                let state = weak.upgrade();
                Box::pin(async move {
                    crate::application::workspace_text::provide(
                        state.ok_or_else(|| Fault {
                            code: "proposal_owner_unavailable".into(),
                            path: "owner".into(),
                        })?,
                        invocation,
                    )
                    .await
                })
            }),
        )
        .map_err(error)?;
        let inputs = proposal::capture(
            kind,
            request,
            task.messages,
            task.max_output_tokens,
            task.length,
        )
        .map_err(error)?;
        let catalog = {
            let mut guard = state.lock()?;
            request.validate(&guard)?;
            let store: &mut Store = &mut guard;
            store
                .workspace_graphs
                .begin(
                    &mut store.connection,
                    &request.install_id,
                    Arc::new(graph),
                    &request.id,
                    inputs,
                    &request.id,
                    request.kind,
                    &serde_json::json!({"scope":{"language":request.language_id}}),
                    |_, _| Ok(()),
                )
                .map_err(error)?
        };
        let values = loop {
            let progress = {
                let mut guard = state.lock()?;
                let authority = request.validate(&guard);
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
                        AppError::new(ErrorCode::Conflict, "Proposal consumer closed.")
                    }));
                }
                let mut domain_error = None;
                store
                    .workspace_graphs
                    .poll(
                        &mut store.connection,
                        &catalog,
                        &request.id,
                        Capacity {
                            local: usize::MAX,
                            provider: admission::NETWORK_CAPACITY,
                        },
                        |db, event| {
                            if matches!(event.intent, CommitIntent::Dispatch { .. }) {
                                graph_identity::bind_workspace(db, event).map_err(|cause| {
                                    domain_error = Some(cause);
                                    Fault { code: "proposal_wire_identity_failed".into(), path: "dispatch".into() }
                                })?;
                            }
                            Ok(())
                        },
                    )
                    .map_err(|fault| domain_error.unwrap_or_else(|| error(fault)))?
            };
            match progress {
                Progress::Waiting => tokio::time::sleep(Duration::from_millis(10)).await,
                Progress::Invoke(invocation) => {
                    let report = invocation
                        .execute(EvidenceLimits {
                            observations: 64,
                            bytes: 1024 * 1024,
                        })
                        .await;
                    let mut guard = state.lock()?;
                    if guard.learner_id()? != request.install_id {
                        return Err(AppError::new(
                            ErrorCode::SessionExpired,
                            "Workspace changed during proposal generation.",
                        ));
                    }
                    let store: &mut Store = &mut guard;
                    store.workspace_graphs.event(&mut store.connection, &catalog, Event::Observe(EvidenceSnapshot {
                        identity: report.identity.clone(), observations: report.observations.clone(), evidence_failure: report.evidence_failure.clone(), provisional: report.provisional.clone(),
                    }), |_, _| Ok(())).map_err(error)?;
                    if let Err(fault) = store.workspace_graphs.event(&mut store.connection, &catalog, Event::SettleObserved(report), |_, _| Ok(())) {
                        let mut failure = error(fault);
                        let receipt = crate::ai::workspace_graph::receipt(&store.connection, &request.id)?;
                        failure.diagnostics = Some(serde_json::json!({"storage":failure.diagnostics,"response":receipt.map(|r| r["response"].clone())}));
                        return Err(failure);
                    }
                }
                Progress::Complete(values) => break values,
                Progress::Failed(cause) => {
                    let mut store = state.lock()?;
                    let (_,_,evidence)=store.workspace_graphs.result_record(&store.connection,&catalog,&request.id,"operation").map_err(error)?;
                    let receipt = crate::ai::workspace_graph::receipt(&store.connection, &request.id)?;
                    let domain=provider_failure(evidence,cause).with_diagnostics(serde_json::json!({"sourceExecution":receipt}));
                    generation::accept_completion(&mut store,request,&Err(domain.clone()))?;
                    return Err(domain);
                }
                Progress::Unknown => {
                    return Err(AppError::new(
                        ErrorCode::UnknownOutcome,
                        "Proposal outcome is unknown. Start a new request explicitly.",
                    ));
                }
                Progress::Cancelled => {
                    return Err(AppError::new(
                        ErrorCode::Conflict,
                        "Proposal was cancelled.",
                    ));
                }
                Progress::Held => {
                    return Err(AppError::new(
                        ErrorCode::AdmissionHeld,
                        "Proposal execution is held.",
                    ));
                }
            }
        };
        let mut store = state.lock()?;
        let (_, _, evidence) = store
            .workspace_graphs
            .result_record(&store.connection, &catalog, &request.id, "operation")
            .map_err(error)?;
        let mut result = provider::Completion {
            text: serde_json::to_string(values.get("candidate").ok_or_else(|| {
                AppError::new(ErrorCode::Storage, "Native proposal candidate is absent.")
            })?)?,
            diagnostics: Some(serde_json::json!({"nativeEvidence":evidence})),
            finish_reason: String::new(),
            actual_model: String::new(),
            provider_id: String::new(),
            input_tokens: None,
            output_tokens: None,
        };
        for observation in evidence.into_iter().flat_map(|e| e.observations) {
            if let Some(value) = observation.request_id {
                result.provider_id = value;
            }
            if let Some(value) = observation.actual_model {
                result.actual_model = value;
            }
            if let Some(value) = observation.finish_reason {
                result.finish_reason = value;
            }
            if let Some(usage) = observation.usage {
                if let Some(value) = usage.input_tokens {
                    result.input_tokens = Some(value.try_into().map_err(|_| {
                        AppError::new(
                            ErrorCode::Storage,
                            "Proposal token count exceeds its contract.",
                        )
                    })?);
                }
                if let Some(value) = usage.output_tokens {
                    result.output_tokens = Some(value.try_into().map_err(|_| {
                        AppError::new(
                            ErrorCode::Storage,
                            "Proposal token count exceeds its contract.",
                        )
                    })?);
                }
            }
        }
        completion = Some(result.clone());
        generation::accept_completion(&mut store, request, &Ok(result.clone()))?;
        request.validate(&store)?;
        parse(&store, request, &result)
    }
    .await;
    if let Err(failure) = &mut outcome {
        let cancellation = (|| -> Result<()> {
            let mut guard = state.lock()?;
            if guard.learner_id()? == request.install_id {
                let store: &mut Store = &mut guard;
                store
                    .workspace_graphs
                    .cancel_run(&mut store.connection, &request.id)
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
    Output {
        completion,
        outcome,
    }
}

/// Domain error evidence remains authoritative for product handling. The graph
/// fault identifies the failed operation; it must not erase refusal semantics.
fn provider_failure(evidence: Option<ExecutionEvidence>, cause: Fault) -> AppError {
    evidence
        .and_then(|e| {
            e.observations.into_iter().rev().find_map(|o| {
                let code =
                    serde_json::from_value::<ErrorCode>(serde_json::json!(o.error_code?)).ok()?;
                let mut error = AppError::new(
                    code,
                    o.redacted_reason
                        .unwrap_or_else(|| "Proposal generation failed.".into()),
                );
                if let Some(EvidenceValue::ClassifiedJson(transport)) =
                    o.additional.get("transport")
                {
                    error.refusal = transport
                        .get("refusal")
                        .and_then(|r| serde_json::from_value(r.clone()).ok());
                }
                Some(error)
            })
        })
        .unwrap_or_else(|| error(cause))
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn native_proposal_failure_preserves_provider_category_and_refusal() {
        let domain = AppError::new(ErrorCode::Provider, "Rate limit reached.").with_refusal(
            crate::model::Refusal {
                reason: crate::model::RefusalReason::RateLimit,
                service_wide: true,
                retry_at: Some(123.0),
                request_id: Some("request-id".into()),
            },
        );
        let observation = crate::ai::transport::graph_evidence::failure(&domain, "model", &[]);
        let evidence = ExecutionEvidence {
            complete: true,
            observations: vec![observation],
            evidence_failure: None,
            provisional: None,
        };
        let result = provider_failure(
            Some(evidence),
            Fault {
                code: "native_failed".into(),
                path: "provider".into(),
            },
        );
        assert_eq!(result.code, ErrorCode::Provider);
        let refusal = result.refusal.unwrap();
        assert!(matches!(
            refusal.reason,
            crate::model::RefusalReason::RateLimit
        ));
        assert!(refusal.service_wide);
        assert_eq!(refusal.retry_at, Some(123.0));
        assert_eq!(refusal.request_id.as_deref(), Some("request-id"));
    }
}
