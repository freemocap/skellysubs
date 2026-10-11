//! Application ownership of native conversation engines. The native artifact/reducer
//! remains the sole topology and scheduling authority.
use super::{coach_graph, graph_authority, graph_publication, prose};
use crate::ai::{
    graph::*,
    graph_store::{self, Partition, TransactionStore},
};
use crate::model::{AppError, ErrorCode, Result};
use rusqlite::{Connection, Transaction, params};
use std::{
    collections::BTreeMap,
    sync::{Arc, OnceLock},
};

mod admission;
mod budget;
pub use admission::Admission;
pub(super) use budget::outstanding;
mod controls;
mod evidence;
mod feedback_requests;
mod help;
mod helper_requests;
pub use help::HelpRequest;
mod execution;
mod message_view;
mod partner_host;
mod playback;
mod speech_requests;
pub use speech_requests::SpeechCommand;
mod reply_validation;
mod retained_views;
mod routing;
mod speech;
mod status;
mod text_authority;
mod usage;
pub use execution::Claim;
pub use usage::Usage;

pub struct Runtime {
    retained_views: BTreeMap<String, BTreeMap<String, InspectionSnapshot>>,
    pub partner: partner_host::PartnerHost,
    pub graph: Arc<Executable>,
    pub catalog: String,
    provider: Arc<OnceLock<prose::Provider>>,
    catalogs: BTreeMap<String, Vec<Arc<Executable>>>,
    partitions: BTreeMap<String, Partition>,
    engines: BTreeMap<String, DurableEngine>,
    audio: BTreeMap<String, speech::PendingAudio>,
    audio_delivery: crate::speech::delivery::DeliveryBuffer,
}

pub fn error(fault: Fault) -> AppError {
    AppError::new(
        ErrorCode::Conflict,
        "Native graph execution could not complete this transition.",
    )
    .with_diagnostics(
        serde_json::json!({"stage":"graph_runtime","code":fault.code,"path":fault.path}),
    )
}

pub fn limits() -> DurableLimits {
    DurableLimits {
        record_reads: RecordReadLimits {
            records: 100_000,
            bytes: 512 * 1024 * 1024,
        },
        state: StateLimits {
            runs: 4096,
            attempts: 32768,
            executions: 32768,
        },
        checkpoint: CheckpointLimits {
            bytes: 32 * 1024 * 1024,
            events: 4096,
        },
        settlement_event_bytes: 2 * 1024 * 1024,
        history: HistoryLimits {
            bytes: 512 * 1024 * 1024,
            events: 100_000,
            segments: 4096,
        },
    }
}

impl Runtime {
    pub fn new() -> Result<Self> {
        let provider: Arc<OnceLock<prose::Provider>> = Arc::new(OnceLock::new());
        let binding = provider.clone();
        let graph = Arc::new(
            coach_graph::compile(Arc::new(move |context, request| {
                let handler = binding.get().cloned();
                Box::pin(async move {
                    match handler {
                        Some(handler) => handler(context, request).await,
                        None => Err(Fault {
                            code: "provider_host_unbound".into(),
                            path: "provider".into(),
                        }),
                    }
                })
            }))
            .map_err(error)?,
        );
        let catalog = graph_store::catalog_id([graph.identity()]).map_err(error)?;
        let mut runtime = Self {
            partner: partner_host::PartnerHost::new()?,
            retained_views: BTreeMap::new(),
            catalogs: BTreeMap::new(),
            partitions: BTreeMap::new(),
            graph,
            catalog,
            provider,
            engines: BTreeMap::new(),
            audio: BTreeMap::new(),
            audio_delivery: Default::default(),
        };
        runtime.register_artifact(runtime.graph.clone())?;
        runtime.register_artifact(runtime.partner.reply.clone())?;
        runtime.register_artifact(runtime.partner.opening.clone())?;
        runtime.register_artifact(runtime.partner.playback.clone())?;
        runtime.register_artifact(runtime.partner.gloss.clone())?;
        runtime.register_artifact(runtime.partner.feedback.clone())?;
        Ok(runtime)
    }

    pub fn bind_provider(&self, provider: prose::Provider) {
        // One application host capability per workspace lifetime; no settings,
        // topology or runtime state is carried in this binding.
        self.provider.get_or_init(|| provider);
    }

    pub fn inspection(
        &self,
        db: &Connection,
        conversation: &str,
        run: &str,
    ) -> Result<Option<InspectionSnapshot>> {
        let engine = self.owner_engine(db, conversation, run)?;
        if let Some(view) = self.retained_view(&engine, run) {
            return Ok(Some(view));
        }
        if !self.owns(db, conversation, run)? {
            return self
                .retained(db, conversation, run)?
                .snapshot(
                    run,
                    ExportLimits {
                        bytes: 4 * 1024 * 1024,
                        attempts: 4096,
                    },
                )
                .map(Some)
                .map_err(error);
        }
        let engine_id = self.owner_engine(db, conversation, run)?;
        self.engines
            .get(&engine_id)
            .map(|engine| {
                engine
                    .inspection_snapshot(
                        run,
                        ExportLimits {
                            bytes: 4 * 1024 * 1024,
                            attempts: 4096,
                        },
                    )
                    .map_err(error)
            })
            .transpose()
    }

    pub fn owns(&self, db: &Connection, conversation: &str, run: &str) -> Result<bool> {
        let id = self.owner_engine(db, conversation, run)?;
        Ok(self.engines.contains_key(&id))
    }

    pub fn preview(
        &self,
        db: &Connection,
        conversation: &str,
        run: &str,
    ) -> Result<Option<AttemptPreview>> {
        if !self.owns(db, conversation, run)? {
            return Ok(None);
        }
        let engine_id = self.owner_engine(db, conversation, run)?;
        let Some(engine) = self.engines.get(&engine_id) else {
            return Ok(None);
        };
        let mut reader = graph_store::ReadStore::from_transaction(
            db.unchecked_transaction()?,
            self.partition(&engine_id)?,
        );
        engine
            .read_node_preview(run, coach_graph::REPLY, 4 * 1024 * 1024, &mut reader)
            .map_err(error)
    }

    pub fn prune(&mut self, db: &Connection) -> Result<()> {
        let existing = db
            .prepare("SELECT id FROM graph_engines")?
            .query_map([], |r| r.get::<_, String>(0))?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        self.engines.retain(|key, _| existing.contains(key));
        self.partitions.retain(|key, _| existing.contains(key));
        Ok(())
    }

    pub fn response(
        &self,
        db: &Connection,
        conversation: &str,
        run: &str,
    ) -> Result<Option<serde_json::Value>> {
        if !self.owns(db, conversation, run)? {
            return Ok(None);
        }
        let engine_id = self.owner_engine(db, conversation, run)?;
        let engine = &self.engines[&engine_id];
        let view = engine.inspect(run).map_err(error)?;
        let Some(attempt) = view.attempts.get(coach_graph::REPLY).and_then(|a| a.last()) else {
            return Ok(None);
        };
        let mut reader = graph_store::ReadStore::from_transaction(
            db.unchecked_transaction()?,
            self.partition(&engine_id)?,
        );
        Ok(engine.read_execution_evidence(attempt.execution,&mut reader).map_err(error)?.map(|evidence|
            serde_json::json!({"node":coach_graph::REPLY,"observations":evidence.observations,"capture_failed":evidence.evidence_failure.is_some(),"complete":evidence.complete})))
    }
}

/// Follow actual artifact references for playback presentation, without changing
/// readiness or manufacturing an independently maintained dependency list.
pub(super) fn ancestors(
    view: &Inspection<'_>,
    node: &str,
) -> Result<std::collections::BTreeSet<String>> {
    definition_ancestors(&view.artifact.definition, node)
}

fn definition_ancestors<V>(
    graph: &Definition<V>,
    node: &str,
) -> Result<std::collections::BTreeSet<String>> {
    let mut seen = std::collections::BTreeSet::new();
    let mut pending = vec![node.to_owned()];
    while let Some(node) = pending.pop() {
        if !seen.insert(node.clone()) {
            continue;
        }
        let definition = graph.nodes.get(&node).ok_or_else(|| {
            AppError::new(
                ErrorCode::Conflict,
                "Native dependency is absent from its artifact.",
            )
        })?;
        pending.extend(definition.after.iter().cloned());
        for source in definition.inputs.values().chain(definition.guard.iter()) {
            if let Source::Output { node, .. } = source {
                pending.push(node.clone());
            }
        }
    }
    Ok(seen)
}
