//! Provider capability for workspace-owned native operations. Exact producer
//! inputs drive the existing transport; credentials and source text stay private.
use super::*;
use crate::ai::{
    graph::*,
    transport::{graph_evidence, graph_identity, graph_request},
};
use crate::model::Result;
fn failed() -> Fault {
    Fault {
        code: "workspace_provider_failed".into(),
        path: "provider".into(),
    }
}
pub(super) async fn provide(
    state: Arc<Application>,
    invocation: InvocationContext,
) -> crate::ai::graph::Result<provider::Completion> {
    let mut key = Zeroizing::new(String::new());
    let mut private = Vec::<String>::new();
    let mut model = String::new();
    let result: Result<provider::Completion> = async {
        let authorize = || {
            let store = state.lock()?;
            let work = store
                .workspace_graphs
                .work(&store.connection, invocation.identity(), |run| {
                    state.reading.validate_native(&store, run).is_ok()
                        || state
                            .generations
                            .native_request(run)
                            .and_then(|r| r.validate(&store))
                            .is_ok()
                })
                .map_err(execution::graph_runtime::error)?;
            let prepared = execution::graph_text_request::prepare(
                &work,
                graph_identity::load_workspace(&store.connection, invocation.identity())?,
            )?;
            if store.snapshot()?.learner.id != prepared.request.install_id {
                return Err(AppError::new(
                    ErrorCode::SessionExpired,
                    "Workspace changed during execution.",
                ));
            }
            access::check_captured(
                &store.connection,
                access::Capability::Chat,
                &prepared.request.target,
            )?;
            if crate::ai::connections::configuration::config(&store.connection)?.paused {
                return Err(AppError::new(
                    ErrorCode::AdmissionHeld,
                    "AI execution is paused.",
                ));
            }
            let proposal = work
                .inputs
                .get("owner")
                .and_then(serde_json::Value::as_str)
                .map(|id| state.generations.native_request(id))
                .transpose()?;
            Ok((prepared, proposal))
        };
        let (prepared, proposal) = authorize()?;
        model = prepared.request.model.clone();
        private = prepared
            .request
            .messages
            .iter()
            .map(|m| m.content.clone())
            .collect();
        let _permit = state.admission.try_chat().ok_or_else(|| {
            AppError::new(
                ErrorCode::AdmissionHeld,
                "AI work is at capacity. Try again when pending work finishes.",
            )
        })?;
        if let Some(credential) = &prepared.request.target.credential {
            key = if let Some(request) = &proposal {
                generation::await_checked(request, read_secret(credential.clone()), || {
                    authorize().map(|_| ())
                })
                .await??
            } else {
                read_secret(credential.clone()).await?
            };
        }
        authorize()?;
        let client = provider::client()?;
        let deltas = grouped::supports_deltas(&client, &key, &prepared.request).await?;
        authorize()?;
        if let Some(request) = &proposal {
            let mut store = state.lock()?;
            request.validate(&store)?;
            generation_receipts::dispatch(&mut store, request)?;
            request.mark_submitted();
        }
        let transport = graph_request::execute(
            &client,
            &key,
            &prepared.request,
            prepared.output(),
            &invocation,
            deltas,
        );
        let output = crate::ai::content::capture::scope(
            state.content_writer(invocation.identity())?,
            transport,
        )
        .await;
        if let Some(error) = output.capture_failure {
            return Err(execution::graph_runtime::error(error));
        }
        match output.item {
            Some(result) => result,
            None => {
                output.transport?;
                Err(AppError::new(
                    ErrorCode::UnknownOutcome,
                    "Provider returned no confirmed result. No automatic retry was made.",
                ))
            }
        }
    }
    .await;
    result.map_err(|error| {
        let mut secrets: Vec<&str> = private.iter().map(String::as_str).collect();
        secrets.push(&key);
        invocation
            .observe(graph_evidence::failure(&error, &model, &secrets))
            .err()
            .unwrap_or_else(failed)
    })
}
