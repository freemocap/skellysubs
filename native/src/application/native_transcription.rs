//! Recording requests use native producer ownership; audio stays in the current
//! capture host, while exact digest/routing inputs and transcript evidence persist.
use super::*;
use crate::ai::{
    graph::*,
    results::{self, Retained},
    transport::graph_identity,
    workspace_graph::{Progress, error},
};
use crate::model::Result;
use crate::speech::transcription_graph as transcription;
use rusqlite::{Connection, OptionalExtension, params};
use serde_json::json;
mod provider;
type Producer = results::pending::Producer<Retained>;
pub(super) struct Host {
    live: std::sync::Weak<Producer>,
    target: access::ResolvedTarget,
    input: audio::TranscriptionRequest,
    install: String,
}
pub(super) type Hosts = std::collections::BTreeMap<String, std::sync::Weak<Host>>;
pub(super) struct Prepared {
    catalog: String,
    key: String,
    _host: Arc<Host>,
}
fn fault() -> Fault {
    Fault {
        code: "native_transcription_failed".into(),
        path: "recording".into(),
    }
}
#[allow(clippy::too_many_arguments)]
pub(super) fn begin(
    state: &Arc<Application>,
    target: access::ResolvedTarget,
    input: audio::TranscriptionRequest,
    install: String,
    key: String,
    producer: &Arc<Producer>,
) -> Result<Prepared> {
    let inputs = transcription::capture(&target, &input, &install)?;
    let host = Arc::new(Host {
        live: Arc::downgrade(producer),
        target,
        input,
        install: install.clone(),
    });
    {
        let mut hosts = state
            .native_transcription_hosts
            .lock()
            .map_err(|_| internal())?;
        hosts.retain(|_, h| h.strong_count() > 0);
        hosts.insert(producer.id().into(), Arc::downgrade(&host));
    }
    let weak = Arc::downgrade(state);
    let graph = Arc::new(
        transcription::compile(Arc::new(move |context, values| {
            let state = weak.upgrade();
            Box::pin(
                async move { provider::provide(&state.ok_or_else(fault)?, context, values).await },
            )
        }))
        .map_err(error)?,
    );
    let mut guard = state.lock()?;
    if guard.learner_id()? != install {
        return Err(AppError::new(
            ErrorCode::SessionExpired,
            "Workspace changed before transcription.",
        ));
    }
    let reuse:String=guard.connection.query_row("SELECT json_extract(r.context,'$.reuseScope') FROM workspace_transcription_cache c JOIN workspace_graph_runs r ON r.run_id=c.run_id WHERE c.request_key=?1 ORDER BY c.last_used DESC LIMIT 1",[&key],|r|r.get(0)).optional()?.unwrap_or_else(||producer.id().into());
    let store: &mut Store = &mut guard;
    let catalog = store
        .workspace_graphs
        .begin(
            &mut store.connection,
            &install,
            graph,
            producer.id(),
            inputs,
            &reuse,
            "transcription",
            &json!({"scope":{"language":host.input.language.language_id},"reuseScope":reuse}),
            |_, _| Ok(()),
        )
        .map_err(error)?;
    Ok(Prepared {
        catalog,
        key,
        _host: host,
    })
}
fn retain(db: &Connection, run: &str, key: &str, values: &Values) -> Result<()> {
    let result: audio::TranscriptionResult = serde_json::from_value(values["transcript"].clone())?;
    let payload = serde_json::to_vec(&result)?;
    if payload.len() as u64 <= results::settings(db)?.capacity_bytes {
        let digest = results::digest(&payload);
        db.execute(
            "INSERT OR IGNORE INTO inference_blobs(digest,payload) VALUES(?1,?2)",
            params![digest, payload],
        )?;
        db.execute("INSERT INTO workspace_transcription_cache(run_id,request_key,blob_digest,last_used) VALUES(?1,?2,?3,?4) ON CONFLICT(run_id) DO UPDATE SET last_used=excluded.last_used",params![run,key,digest,results::tick(db)?])?;
        results::prune(db)?;
    }
    Ok(())
}
pub(super) async fn execute(
    state: &Arc<Application>,
    install: &str,
    producer: &Arc<Producer>,
    prepared: Prepared,
) -> Result<Retained> {
    let result = execute_inner(state, install, producer, &prepared).await;
    if result.is_err() {
        let mut guard = state.lock()?;
        if guard.learner_id()? == install {
            let store: &mut Store = &mut guard;
            store
                .workspace_graphs
                .cancel_run(&mut store.connection, producer.id())
                .map_err(error)?;
        }
    }
    result
}
async fn execute_inner(
    state: &Arc<Application>,
    install: &str,
    producer: &Arc<Producer>,
    prepared: &Prepared,
) -> Result<Retained> {
    let mut settling = false;
    loop {
        let progress = {
            let mut guard = state.lock()?;
            if guard.learner_id()? != install {
                return Err(AppError::new(
                    ErrorCode::SessionExpired,
                    "Workspace changed during transcription.",
                ));
            }
            if !settling && !producer.has_subscribers() {
                return Err(AppError::new(
                    ErrorCode::Conflict,
                    "Recording consumers closed.",
                ));
            }
            let paused = crate::ai::connections::configuration::config(&guard.connection)?.paused;
            let store: &mut Store = &mut guard;
            store
                .workspace_graphs
                .poll(
                    &mut store.connection,
                    &prepared.catalog,
                    producer.id(),
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
                            graph_identity::bind_workspace(db, event).map_err(|_| fault())?;
                        }
                        Ok(())
                    },
                )
                .map_err(error)?
        };
        match progress {
            Progress::Waiting => tokio::time::sleep(Duration::from_millis(10)).await,
            Progress::Invoke(invocation) => {
                // The provider checks consumers before dispatch. Once invoked,
                // finish settlement/adoption even if every recording closes;
                // consumer validation still prevents publishing to closed owners.
                settling = true;
                let report = invocation
                    .execute(EvidenceLimits {
                        observations: 64,
                        bytes: 1024 * 1024,
                    })
                    .await;
                let mut guard = state.lock()?;
                if guard.learner_id()? != install {
                    return Err(AppError::new(
                        ErrorCode::SessionExpired,
                        "Workspace changed during transcription.",
                    ));
                }
                let store: &mut Store = &mut guard;
                store
                    .workspace_graphs
                    .event(
                        &mut store.connection,
                        &prepared.catalog,
                        Event::Observe(EvidenceSnapshot {
                            identity: report.identity.clone(),
                            observations: report.observations.clone(),
                            evidence_failure: report.evidence_failure.clone(),
                            provisional: report.provisional.clone(),
                        }),
                        |_, _| Ok(()),
                    )
                    .map_err(error)?;
                let values = report.outcome.as_ref().ok().cloned();
                let mut storage_error = None;
                let settled = store.workspace_graphs.event(
                    &mut store.connection,
                    &prepared.catalog,
                    Event::SettleObserved(report.clone()),
                    |db, _| {
                        if let Some(values) = &values
                            && let Err(cause) = retain(db, producer.id(), &prepared.key, values)
                        {
                            storage_error = Some(cause);
                            return Err(fault());
                        }
                        Ok(())
                    },
                );
                if let Err(cause) = settled {
                    let mut failed = report;
                    failed.outcome = Err(fault());
                    store
                        .workspace_graphs
                        .event(
                            &mut store.connection,
                            &prepared.catalog,
                            Event::SettleObserved(failed),
                            |_, _| Ok(()),
                        )
                        .map_err(error)?;
                    return Err(storage_error.unwrap_or_else(||error(cause)).with_diagnostics(json!({"sourceReceipt":crate::ai::workspace_graph::receipt(&store.connection,producer.id())?.map(|r|r["response"].clone())})));
                }
            }
            Progress::Complete(values) => {
                let store = state.lock()?;
                crate::ai::connections::speech_routing::validate_access(
                    &store.connection,
                    access::Capability::Transcription,
                    &prepared._host.target,
                )?;
                let tx = store.connection.unchecked_transaction()?;
                retain(&tx, producer.id(), &prepared.key, &values)?;
                tx.commit()?;
                let (engine, attempt, _) = store
                    .workspace_graphs
                    .result_record(
                        &store.connection,
                        &prepared.catalog,
                        producer.id(),
                        "operation",
                    )
                    .map_err(error)?;
                let metadata =
                    crate::ai::workspace_graph::receipt(&store.connection, producer.id())?
                        .ok_or_else(internal)?["response"]
                        .clone();
                return Ok(Retained {
                    cached: attempt.acquisition != Acquisition::Produced,
                    execution: format!(
                        "graph:{}",
                        serde_json::to_string(&(engine, attempt.execution))?
                    ),
                    payload: serde_json::to_vec(&values["transcript"])?,
                    metadata,
                });
            }
            Progress::Failed(cause) => {
                let store = state.lock()?;
                let (_, _, evidence) = store
                    .workspace_graphs
                    .result_record(
                        &store.connection,
                        &prepared.catalog,
                        producer.id(),
                        "operation",
                    )
                    .map_err(error)?;
                let domain = evidence
                    .and_then(|e| {
                        e.observations.into_iter().rev().find_map(|o| {
                            Some(AppError::new(
                                serde_json::from_value::<ErrorCode>(json!(o.error_code?)).ok()?,
                                o.redacted_reason
                                    .unwrap_or_else(|| "Transcription failed.".into()),
                            ))
                        })
                    })
                    .unwrap_or_else(|| error(cause));
                return Err(domain.with_diagnostics(json!({"sourceReceipt":crate::ai::workspace_graph::receipt(&store.connection,producer.id())?.map(|r|r["response"].clone())})));
            }
            Progress::Held => {
                return Err(AppError::new(
                    ErrorCode::AdmissionHeld,
                    "AI execution is paused.",
                ));
            }
            Progress::Unknown => {
                return Err(AppError::new(
                    ErrorCode::UnknownOutcome,
                    "Transcription outcome is unknown. Retry explicitly.",
                ));
            }
            Progress::Cancelled => {
                return Err(AppError::new(
                    ErrorCode::Conflict,
                    "Transcription cancelled.",
                ));
            }
        }
    }
}
