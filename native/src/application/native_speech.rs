//! Native playback admission. Subscriptions fan out one explicit request; graph
//! readiness, provider dispatch and receipt settlement remain native operations.
use super::*;
use crate::ai::{
    graph::*,
    results::Retained,
    transport::graph_identity,
    workspace_graph::{Progress, error},
};
use crate::language::source_graph::SourceText;
use crate::model::Result;
use crate::speech::{alignment::SpeechAudio, graph_audio, synthesis_graph as synthesis};
use rusqlite::Connection;
use serde_json::json;
mod provider;
use provider::provide;
type Producer = crate::ai::results::pending::Producer<Retained>;
struct Payload {
    receipt: synthesis::Receipt,
    wav: Vec<u8>,
    alignment: Option<crate::speech::alignment::SpeechAlignment>,
    identity: Option<InvocationIdentity>,
    request: synthesis::Request,
}
pub(super) struct Host {
    live: std::sync::Weak<Producer>,
    payload: Arc<Mutex<Option<Payload>>>,
}
pub(super) type Hosts = std::collections::BTreeMap<String, std::sync::Weak<Host>>;
fn host(state: &Application, identity: &InvocationIdentity) -> Result<Arc<Host>> {
    let store = state.lock()?;
    let hosts = state.native_speech_hosts.lock().map_err(|_| internal())?;
    let mut chosen = None;
    store
        .workspace_graphs
        .work(&store.connection, identity, |run| {
            if let Some(host) = hosts.get(run).and_then(std::sync::Weak::upgrade) {
                chosen = Some(host);
                true
            } else {
                false
            }
        })
        .map_err(error)?;
    chosen.ok_or_else(internal)
}
pub(super) struct Prepared {
    _host: Arc<Host>,
    catalog: String,
    payload: Arc<Mutex<Option<Payload>>>,
}
fn fault() -> Fault {
    Fault {
        code: "workspace_speech_failed".into(),
        path: "speech".into(),
    }
}
pub(super) fn associate(db: &Connection, consumer: &str, run: &str) -> Result<()> {
    db.execute("INSERT INTO workspace_graph_consumers(consumer_id,run_id) VALUES(?1,?2) ON CONFLICT(consumer_id) DO UPDATE SET run_id=excluded.run_id,stream_id=NULL", rusqlite::params![consumer,run])?;
    Ok(())
}
pub(super) fn begin(
    state: &Arc<Application>,
    target: &access::ResolvedTarget,
    input: &audio::SpeechInput,
    install: &str,
    producer: &Arc<Producer>,
) -> Result<Prepared> {
    let payload = Arc::new(Mutex::new(None));
    let weak = Arc::downgrade(state);
    let owner = Arc::new(Host {
        live: Arc::downgrade(producer),
        payload: payload.clone(),
    });
    {
        let mut hosts = state.native_speech_hosts.lock().map_err(|_| internal())?;
        hosts.retain(|_, h| h.strong_count() > 0);
        hosts.insert(producer.id().into(), Arc::downgrade(&owner));
    }
    let provider = Arc::new(
        move |context: InvocationContext, request: synthesis::Request| {
            let state = weak.upgrade();
            Box::pin(async move {
                let state = state.ok_or_else(fault)?;
                let host = host(&state, context.identity()).map_err(|_| fault())?;
                provide(
                    &state,
                    context,
                    request,
                    host.live.clone(),
                    host.payload.clone(),
                )
                .await
            })
                as std::pin::Pin<
                    Box<
                        dyn std::future::Future<
                                Output = crate::ai::graph::Result<synthesis::Receipt>,
                            > + Send,
                    >,
                >
        },
    );
    let weak = Arc::downgrade(state);
    let lookup = Arc::new(
        move |context: InvocationContext, request: synthesis::Request| {
            let state = weak.upgrade();
            Box::pin(async move {
                let result: Result<_> = (|| {
                    let state = state.ok_or_else(internal)?;
                    let owner = host(&state, context.identity())?;
                    let store = state.lock()?;
                    // Cache lookup needs valid captured access, but may run while paused.
                    if store.learner_id()? != request.settings.install_id {
                        return Err(internal());
                    }
                    crate::ai::connections::speech_routing::validate_access(
                        &store.connection,
                        access::Capability::Speech,
                        &request.settings.target,
                    )?;
                    let tx = store.connection.unchecked_transaction()?;
                    let cached = graph_audio::lookup(&tx, &request)?;
                    tx.commit()?;
                    if let Some(cached) = cached {
                        let receipt = cached.receipt.clone();
                        let run = owner.live.upgrade().ok_or_else(internal)?;
                        store.connection.execute(
                            "UPDATE workspace_graph_consumers SET stream_id=?2 WHERE run_id=?1",
                            rusqlite::params![run.id(), receipt.id],
                        )?;
                        *owner.payload.lock().map_err(|_| internal())? = Some(Payload {
                            receipt: cached.receipt,
                            wav: cached.wav,
                            alignment: cached.alignment,
                            identity: None,
                            request,
                        });
                        Ok(Some(receipt))
                    } else {
                        Ok(None)
                    }
                })();
                result.map_err(|cause| {
                    context
                        .observe(crate::ai::transport::graph_evidence::failure(
                            &cause,
                            "",
                            &[],
                        ))
                        .err()
                        .unwrap_or_else(fault)
                })
            })
                as std::pin::Pin<
                    Box<
                        dyn std::future::Future<
                                Output = crate::ai::graph::Result<Option<synthesis::Receipt>>,
                            > + Send,
                    >,
                >
        },
    );
    let graph = Arc::new(synthesis::playback::compile(provider, lookup).map_err(error)?);
    let settings = synthesis::Settings {
        target: target.clone(),
        install_id: install.into(),
        language_tag: input.language_tag.clone(),
        language: input.language.clone(),
        voice: input.voice.clone(),
    };
    let key = crate::ai::results::speech::request_key(
        &crate::ai::results::speech::scope(target, install)?,
        input,
    )?;
    let mut inputs = synthesis::capture(
        SourceText {
            id: key,
            text: input.text.clone(),
        },
        settings,
    )
    .map_err(error)?;
    inputs.insert("regenerate".into(), json!(false));
    let mut guard = state.lock()?;
    if guard.learner_id()? != install {
        return Err(AppError::new(
            ErrorCode::SessionExpired,
            "Workspace changed before speech.",
        ));
    }
    let store: &mut Store = &mut guard;
    let catalog = store
        .workspace_graphs
        .begin(
            &mut store.connection,
            install,
            graph,
            producer.id(),
            inputs,
            producer.id(),
            "speech",
            &json!({"scope":{"languageTag":input.language_tag}}),
            |_, _| Ok(()),
        )
        .map_err(error)?;
    store
        .workspace_graphs
        .event(
            &mut store.connection,
            &catalog,
            Event::Demand {
                run: producer.id().into(),
                node: "lookup".into(),
            },
            |_, _| Ok(()),
        )
        .map_err(error)?;
    Ok(Prepared {
        catalog,
        payload,
        _host: owner,
    })
}
pub(super) async fn execute(
    state: &Arc<Application>,
    install: &str,
    producer: &Arc<Producer>,
    prepared: Prepared,
) -> Result<Retained> {
    let result = execute_inner(state, install, producer, &prepared).await;
    prepared.payload.lock().map_err(|_| internal())?.take();
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
    // At most four 4-MiB payloads may be pinned across lookup and settlement.
    let _slot = state
        .native_audio_slots
        .acquire()
        .await
        .map_err(|_| internal())?;
    loop {
        let progress = {
            let mut guard = state.lock()?;
            if guard.learner_id()? != install {
                return Err(AppError::new(
                    ErrorCode::SessionExpired,
                    "Workspace changed during speech.",
                ));
            }
            if !producer.has_subscribers() {
                return Err(AppError::new(
                    ErrorCode::Conflict,
                    "Speech consumers closed.",
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
                        if let CommitIntent::Dispatch { work, .. } = &event.intent
                            && work.resource == Resource::Provider
                        {
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
                        "Workspace changed during speech.",
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
                let synthesis = report.identity.operation == synthesis::operation_contract();
                let success = report.outcome.is_ok();
                let mut storage_error = None;
                let settled = store.workspace_graphs.event(
                    &mut store.connection,
                    &prepared.catalog,
                    Event::SettleObserved(report.clone()),
                    |db, _| {
                        if synthesis && success {
                            let payload = prepared.payload.lock().map_err(|_| fault())?;
                            let p = payload.as_ref().ok_or_else(fault)?;
                            if let Err(cause) = graph_audio::retain(
                                db,
                                p.identity.as_ref().ok_or_else(fault)?,
                                &p.request,
                                &p.wav,
                                p.alignment.as_ref(),
                            ) {
                                storage_error = Some(cause);
                                return Err(fault());
                            }
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
                    let receipt =
                        crate::ai::workspace_graph::receipt(&store.connection, producer.id())?;
                    return Err(storage_error
                        .unwrap_or_else(|| error(cause))
                        .with_diagnostics(
                            json!({"response":receipt.map(|r|r["response"].clone())}),
                        ));
                }
            }
            Progress::Complete(values) => {
                let payload = prepared
                    .payload
                    .lock()
                    .map_err(|_| internal())?
                    .take()
                    .ok_or_else(internal)?;
                if values["audio"]["receipt"] != serde_json::to_value(&payload.receipt)? {
                    return Err(internal());
                }
                let store = state.lock()?;
                let tx = store.connection.unchecked_transaction()?;
                graph_audio::verify(&tx, &payload.receipt)?;
                tx.commit()?;
                let metadata = crate::ai::workspace_graph::execution_receipt(
                    &store.connection,
                    &payload.receipt.engine,
                    serde_json::from_str(&payload.receipt.execution)?,
                )?["response"]
                    .clone();
                return Ok(Retained {
                    cached: payload.identity.is_none(),
                    execution: payload.receipt.id,
                    payload: serde_json::to_vec(&SpeechAudio::new(
                        &payload.wav,
                        payload.alignment,
                    ))?,
                    metadata,
                });
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
                    "Speech outcome is unknown. Retry explicitly.",
                ));
            }
            Progress::Cancelled => {
                return Err(AppError::new(
                    ErrorCode::Conflict,
                    "Speech request was cancelled.",
                ));
            }
            Progress::Failed(cause) => {
                let store = state.lock()?;
                return Err(error(cause).with_diagnostics(json!({"response":crate::ai::workspace_graph::receipt(&store.connection,producer.id())?.map(|r|r["response"].clone())})));
            }
        }
    }
}
