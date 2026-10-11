//! Host capabilities for executable conversation graphs. Native claim/settlement
//! owns scheduling facts; this adapter owns permits, secrets and asynchronous I/O.
use super::*;
use crate::ai::{
    graph,
    transport::{graph_evidence, graph_request},
};
use execution::{graph_runtime, prose};

fn fault() -> graph::Fault {
    graph::Fault {
        code: "conversation_provider_failed".into(),
        path: "provider".into(),
    }
}

async fn provide(
    state: Arc<Application>,
    client: reqwest::Client,
    invocation: graph::InvocationContext,
    captured: prose::Request,
) -> graph::Result<String> {
    let completion = provide_text(state, client, invocation.clone()).await?;
    prose::prepare_reply(&completion, &invocation).map_err(|cause| {
        let mut private: Vec<&str> = captured
            .context
            .messages
            .iter()
            .map(|m| m.content.as_str())
            .collect();
        private.push(&completion.text);
        invocation
            .observe(graph_evidence::failure(
                &cause,
                &captured.target.model,
                &private,
            ))
            .err()
            .unwrap_or_else(fault)
    })
}

async fn provide_text(
    state: Arc<Application>,
    client: reqwest::Client,
    invocation: graph::InvocationContext,
) -> graph::Result<provider::Completion> {
    let mut key = Zeroizing::new(String::new());
    let mut model = None;
    let mut private_content = Vec::new();
    let result: Result<provider::Completion> = async {
        let authorize = || {
            let store = state.lock()?;
            store
                .graph_runtime
                .authorize_text(&store.connection, invocation.identity())
        };
        let prepared = authorize()?;
        model = Some(prepared.request.model.clone());
        private_content.extend(prepared.request.messages.iter().map(|m| m.content.clone()));
        if let Some(credential) = &prepared.request.target.credential {
            key = read_secret(credential.clone()).await?;
        }
        authorize()?;
        let request = &prepared.request;
        let deltas = grouped::supports_deltas(&client, &key, request).await?;
        authorize()?;
        let transport = graph_request::execute(
            &client,
            &key,
            request,
            prepared.output(),
            &invocation,
            deltas,
        );
        tokio::pin!(transport);
        let mut retained = None;
        let outcome = loop {
            tokio::select! {
                outcome = &mut transport => break outcome,
                _ = tokio::time::sleep(Duration::from_millis(250)) => {
                    let snapshot = invocation.snapshot().map_err(graph_runtime::error)?;
                    if retained.as_ref() != Some(&snapshot) {
                        let mut guard = state.lock()?;
                        let store: &mut Store = &mut guard;
                        store.graph_runtime.observe(&mut store.connection, snapshot.clone())?;
                        retained = Some(snapshot);
                        drop(guard);
                        state.conversation_changed.notify_waiters();
                    }
                }
            }
        };
        if outcome.capture_failure.is_some() {
            return Err(AppError::new(
                ErrorCode::Storage,
                "Provider evidence could not be captured.",
            ));
        }
        // A confirmed item remains useful even if the enclosing stream failed;
        // graph_request has already retained both observations in arrival order.
        let completion = match outcome.item {
            Some(item) => item?,
            None => {
                outcome.transport?;
                return Err(AppError::new(
                    ErrorCode::UnknownOutcome,
                    "Provider returned no confirmed result. No automatic retry was made.",
                ));
            }
        };
        invocation
            .provisional_text(&completion.text)
            .map_err(graph_runtime::error)?;
        Ok(completion)
    }
    .await;
    result.map_err(|error| {
        let mut private: Vec<&str> = private_content.iter().map(String::as_str).collect();
        private.push(&key);
        // Includes failures before HTTP; no requested model is invented if the
        // producer could not be read/authorized in the first place.
        let mut evidence =
            graph_evidence::failure(&error, model.as_deref().unwrap_or_default(), &private);
        evidence.requested_model = model;
        invocation.observe(evidence).err().unwrap_or_else(fault)
    })
}

pub(super) async fn schedule(state: &Arc<Application>, client: &reqwest::Client) -> Result<()> {
    // Drain ready work without a node-count quota. Yield after each launch so
    // completions and UI reads can run; this introduces no polling delay or cap.
    while schedule_one(state, client)? {
        tokio::task::yield_now().await;
    }
    Ok(())
}

fn schedule_one(state: &Arc<Application>, client: &reqwest::Client) -> Result<bool> {
    let mut permit = state.admission.try_chat();
    let (claim, session) = {
        let mut guard = state.lock()?;
        let store: &mut Store = &mut guard;
        super::graph_speech::bind(state, store, client);
        let text_state = Arc::downgrade(state);
        let evidence_state = Arc::downgrade(state);
        let text_client = client.clone();
        store.graph_runtime.partner.bind(
            Arc::new(move |context| {
                let state = text_state.upgrade();
                let client = text_client.clone();
                Box::pin(
                    async move { provide_text(state.ok_or_else(fault)?, client, context).await },
                )
            }),
            Arc::new(move |context, source| {
                let state = evidence_state.upgrade();
                Box::pin(async move {
                    let result = (|| -> Result<_> {
                        let state = state.ok_or_else(|| {
                            AppError::new(ErrorCode::Conflict, "Workspace is unavailable.")
                        })?;
                        let store = state.lock()?;
                        store.graph_runtime.available_evidence(
                            &store.connection,
                            context.identity(),
                            &source,
                        )
                    })();
                    result.map_err(|cause| {
                        context
                            .observe(graph_evidence::failure(&cause, "local", &[&source.text]))
                            .err()
                            .unwrap_or_else(fault)
                    })
                })
            }),
        );
        let weak = Arc::downgrade(state);
        let client = client.clone();
        store
            .graph_runtime
            .bind_provider(Arc::new(move |context, request| {
                let state = weak.upgrade();
                let client = client.clone();
                Box::pin(async move {
                    provide(state.ok_or_else(fault)?, client, context, request).await
                })
            }));
        let claim = store.graph_runtime.next(
            &mut store.connection,
            permit.is_some(),
            &store.config,
            &store.session_id,
        )?;
        (claim, store.session_id.clone())
    };
    let Some(claim) = claim else {
        return Ok(false);
    };
    if claim.resource == graph::Resource::Local {
        permit.take();
    }
    let state = state.clone();
    tauri::async_runtime::spawn(async move {
        let graph_runtime::Claim {
            conversation,
            run,
            invocation,
            ..
        } = claim;
        let report = invocation
            .execute_with_provisional(
                graph::EvidenceLimits {
                    observations: 64,
                    bytes: 512 * 1024,
                },
                graph::ProvisionalLimits { bytes: 512 * 1024 },
            )
            .await;
        let result = state.lock().and_then(|mut guard| {
            let store: &mut Store = &mut guard;
            if store.session_id != session {
                return Ok(());
            }
            if !store.connection.query_row(
                "SELECT EXISTS(SELECT 1 FROM conversations WHERE id=?1)",
                [&conversation],
                |r| r.get::<_, bool>(0),
            )? {
                return Ok(()); // The learner deleted the owning conversation.
            }
            store
                .graph_runtime
                .finish(&mut store.connection, &conversation, &run, report.clone())
        });
        drop(permit);
        state.scheduler_wake.notify_waiters();
        state.conversation_changed.notify_waiters();
        if let Err(error) = result {
            state
                .graph_reports
                .lock()
                .unwrap_or_else(|poison| poison.into_inner())
                .push(report);
            state.stop(error);
        }
    });
    Ok(true)
}

#[cfg(test)]
#[path = "tests/native_partner.rs"]
mod tests;
