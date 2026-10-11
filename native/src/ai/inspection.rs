//! Read-only native graph interrogation. Persisted ownership chooses the partition;
//! the core supplies topology, attempts and evidence at the requested revision.
use super::{
    graph::*,
    graph_store::{self, Owner, Partition},
};
use crate::model::{AppError, ErrorCode, Result};
use rusqlite::{Connection, OptionalExtension, params};
use serde::Serialize;
use ts_rs::TS;
pub mod cache;
mod content;

#[derive(Serialize, serde::Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct NativeAttemptInspection {
    pub engine: String,
    pub run: String,
    pub artifact: String,
    pub revision: String,
    pub node: String,
    pub attempt: String,
    pub execution: String,
    pub owner: String,
    #[ts(type = "unknown")]
    pub evidence: Option<serde_json::Value>,
    pub retained_text: Option<String>,
    #[ts(type = "unknown")]
    pub inputs: serde_json::Value,
    #[ts(type = "unknown")]
    pub outcome: Option<serde_json::Value>,
}

fn invalid(message: &str) -> AppError {
    AppError::new(ErrorCode::Validation, message)
}
fn graph_error(fault: Fault) -> AppError {
    AppError::new(ErrorCode::Storage, "Native graph inspection failed.")
        .with_diagnostics(serde_json::json!({"code":fault.code,"path":fault.path}))
}
fn number(value: &str) -> Result<u64> {
    value
        .parse::<u64>()
        .ok()
        .filter(|n| n.to_string() == value)
        .ok_or_else(|| invalid("Invalid graph revision or execution identity."))
}
fn partition(db: &Connection, engine: &str, run: &str) -> Result<Partition> {
    let conversation: Option<(String,String)> = db.query_row(
        "SELECT e.conversation_id,e.catalog FROM graph_engines e JOIN graph_conversation_runs r ON r.engine_id=e.id JOIN turns t ON t.id=r.turn_id AND t.conversation_id=e.conversation_id WHERE e.id=?1 AND r.run_id=?2",
        params![engine,run], |r| Ok((r.get(0)?,r.get(1)?))).optional()?;
    if let Some((id, catalog)) = conversation {
        return Ok(Partition {
            owner: Owner::Conversation(id),
            catalog,
        });
    }
    let workspace: Option<(String,String)> = db.query_row(
        "SELECT e.workspace_id,e.catalog FROM workspace_graph_engines e JOIN workspace_graph_runs r ON r.engine_id=e.id JOIN learner l ON l.id=e.workspace_id WHERE e.id=?1 AND r.run_id=?2",
        params![engine,run], |r| Ok((r.get(0)?,r.get(1)?))).optional()?;
    workspace
        .map(|(id, catalog)| Partition {
            owner: Owner::Workspace(id),
            catalog,
        })
        .ok_or_else(|| AppError::new(ErrorCode::NotFound, "Graph run owner is unavailable."))
}
fn checkpoint(reader: &graph_store::BorrowedReadStore<'_>) -> Result<Checkpoint> {
    reader
        .checkpoint(CheckpointLimits {
            bytes: 32 * 1024 * 1024,
            events: 4096,
        })
        .map_err(graph_error)?
        .ok_or_else(|| AppError::new(ErrorCode::Storage, "Graph checkpoint is missing."))
}
fn limits() -> HistoricalLimits {
    HistoricalLimits {
        history: HistoryLimits {
            bytes: 512 * 1024 * 1024,
            events: 100_000,
            segments: 4096,
        },
        state: StateLimits {
            runs: 4096,
            attempts: 32768,
            executions: 32768,
        },
    }
}
fn export() -> ExportLimits {
    ExportLimits {
        bytes: 8 * 1024 * 1024,
        attempts: 4096,
    }
}

pub struct PreparedHistory {
    pub owner: String,
    pub source: graph_store::DetachedHistory,
}

pub fn prepare(db: &Connection, engine: &str, run: &str) -> Result<PreparedHistory> {
    let tx = db.unchecked_transaction()?;
    let partition = partition(&tx, engine, run)?;
    let source =
        graph_store::DetachedHistory::capture(&tx, &partition, 32 * 1024 * 1024, limits().history)
            .map_err(graph_error)?;
    if source.stamp().engine != engine {
        return Err(invalid("Graph engine identity differs."));
    }
    Ok(PreparedHistory {
        owner: partition.owner.key().into(),
        source,
    })
}

#[cfg(test)]
pub fn attempt(
    db: &Connection,
    engine: &str,
    run: &str,
    revision: &str,
    node: &str,
    attempt: &str,
) -> Result<NativeAttemptInspection> {
    prepare(db, engine, run)?.attempt(engine, run, revision, node, attempt)
}

impl PreparedHistory {
    pub fn workspace_receipt(
        mut self,
        engine: &str,
        run: &str,
        execution: Option<ExecutionId>,
    ) -> Result<Option<serde_json::Value>> {
        let checkpoint = self
            .source
            .checkpoint(CheckpointLimits {
                bytes: 32 * 1024 * 1024,
                events: 4096,
            })
            .map_err(graph_error)?;
        let history = checkpoint
            .historical_inspection(checkpoint.stamp().revision, limits(), &mut self.source)
            .map_err(graph_error)?;
        if let Some(execution) = execution {
            let mut receipt = super::workspace_graph::history::execution_receipt_from_history(
                &history, engine, execution,
            )?;
            receipt["nativeRun"] = serde_json::json!(run);
            Ok(Some(receipt))
        } else {
            super::workspace_graph::history::receipt_from_history(&history, engine, run)
        }
    }
    pub fn current_snapshots(
        mut self,
        runs: &[String],
    ) -> Result<std::collections::BTreeMap<String, InspectionSnapshot>> {
        let checkpoint = self
            .source
            .checkpoint(CheckpointLimits {
                bytes: 32 * 1024 * 1024,
                events: 4096,
            })
            .map_err(graph_error)?;
        let history = checkpoint
            .historical_inspection(checkpoint.stamp().revision, limits(), &mut self.source)
            .map_err(graph_error)?;
        runs.iter()
            .map(|run| {
                Ok((
                    run.clone(),
                    history.snapshot(run, export()).map_err(graph_error)?,
                ))
            })
            .collect()
    }
    pub fn cache_identity(&self) -> Result<String> {
        self.source.cache_identity().map_err(graph_error)
    }
    pub fn attempt(
        mut self,
        engine: &str,
        run: &str,
        revision: &str,
        node: &str,
        attempt: &str,
    ) -> Result<NativeAttemptInspection> {
        let owner = self.owner;
        let checkpoint = self
            .source
            .checkpoint(CheckpointLimits {
                bytes: 32 * 1024 * 1024,
                events: 4096,
            })
            .map_err(graph_error)?;
        let history = checkpoint
            .historical_inspection(number(revision)?, limits(), &mut self.source)
            .map_err(graph_error)?;
        let snapshot = history.snapshot(run, export()).map_err(graph_error)?;
        let selected = snapshot
            .attempts
            .get(node)
            .and_then(|items| items.iter().find(|item| item.id == attempt))
            .ok_or_else(|| invalid("Attempt does not belong to the selected node and revision."))?;
        let execution: ExecutionId =
            serde_json::from_value(serde_json::json!(number(&selected.execution)?))?;
        let evidence = history.execution_evidence(execution).map_err(graph_error)?;
        let (work, outcome) = history.execution_content(execution).map_err(graph_error)?;
        let inputs = content::inputs(work.inputs);
        let outcome = outcome.map(serde_json::to_value).transpose()?;
        let retained_text = evidence
            .as_ref()
            .and_then(|value| value.provisional.as_ref())
            .map(|value| value.text.clone());
        let evidence = evidence.map(|value| serde_json::json!({"observations":value.observations,"complete":value.complete,"capture_failed":value.evidence_failure.is_some() || value.provisional.as_ref().is_some_and(|p|p.failure.is_some()),"provisional_capture":value.provisional.as_ref().map(|p|serde_json::json!({"session":p.session,"sequence":p.sequence,"failure":p.failure}))}));
        Ok(NativeAttemptInspection {
            engine: engine.into(),
            run: run.into(),
            artifact: snapshot.artifact_id,
            revision: revision.into(),
            node: node.into(),
            attempt: attempt.into(),
            execution: selected.execution.clone(),
            owner,
            evidence,
            retained_text,
            inputs,
            outcome,
        })
    }
}

#[derive(Debug, Clone, Serialize, serde::Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct NativeRunEntry {
    pub engine: String,
    pub run: String,
    pub artifact: String,
    pub kind: String,
}
#[derive(Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct NativeRunPage {
    pub runs: Vec<NativeRunEntry>,
    pub before: Option<String>,
}

pub fn workspace_runs(db: &Connection, before: Option<&str>) -> Result<NativeRunPage> {
    if before.is_some_and(|value| value.is_empty() || value.len() > 2048) {
        return Err(invalid("Invalid graph list cursor."));
    }
    // Stable identity order makes paging independent of unrelated activity. This
    // is a run catalog; chronological execution is represented by each timeline.
    let mut query = db.prepare("WITH runs AS (SELECT r.engine_id,r.run_id,r.artifact_id,r.kind FROM workspace_graph_runs r JOIN workspace_graph_engines e ON e.id=r.engine_id JOIN learner l ON l.id=e.workspace_id UNION SELECT r.engine_id,r.run_id,r.artifact_id,r.channel FROM graph_conversation_runs r JOIN graph_engines e ON e.id=r.engine_id JOIN turns t ON t.id=r.turn_id AND t.conversation_id=e.conversation_id) SELECT engine_id || ':' || run_id,engine_id,run_id,artifact_id,kind FROM runs WHERE (?1 IS NULL OR engine_id || ':' || run_id<?1) ORDER BY engine_id || ':' || run_id DESC LIMIT 51")?;
    let mut rows = query
        .query_map([before], |r| {
            Ok((
                r.get::<_, String>(0)?,
                NativeRunEntry {
                    engine: r.get(1)?,
                    run: r.get(2)?,
                    artifact: r.get(3)?,
                    kind: r.get(4)?,
                },
            ))
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    let more = rows.len() > 50;
    rows.truncate(50);
    let before = if more {
        rows.last().map(|(id, _)| id.clone())
    } else {
        None
    };
    Ok(NativeRunPage {
        runs: rows.into_iter().map(|(_, entry)| entry).collect(),
        before,
    })
}

#[cfg(test)]
pub fn history(
    db: &Connection,
    engine: &str,
    run: &str,
    before: Option<&str>,
) -> Result<RunHistory> {
    prepare(db, engine, run)?.history(run, before, 32)
}
impl PreparedHistory {
    pub fn history(mut self, run: &str, before: Option<&str>, count: u32) -> Result<RunHistory> {
        if !(1..=64).contains(&count) {
            return Err(invalid("Graph history page size must be between 1 and 64."));
        }
        let checkpoint = self
            .source
            .checkpoint(CheckpointLimits {
                bytes: 32 * 1024 * 1024,
                events: 4096,
            })
            .map_err(graph_error)?;
        checkpoint
            .run_history(
                run,
                before.map(number).transpose()?,
                count as usize,
                limits(),
                export(),
                &mut self.source,
            )
            .map_err(graph_error)
    }
}

pub fn current(
    db: &Connection,
    runtime: &super::workspace_graph::Runtime,
    conversations: &crate::conversations::execution::graph_runtime::Runtime,
    engine: &str,
    run: &str,
    after: Option<&str>,
) -> Result<Option<InspectionSnapshot>> {
    let partition = partition(db, engine, run)?;
    let workspace = matches!(partition.owner, Owner::Workspace(_));
    let raw: String = db.query_row(
        if workspace {
            "SELECT stamp FROM workspace_graph_engines WHERE id=?1"
        } else {
            "SELECT stamp FROM graph_engines WHERE id=?1"
        },
        [engine],
        |r| r.get(0),
    )?;
    let stamp: Stamp = serde_json::from_str(&raw)?;
    if after.map(number).transpose()? == Some(stamp.revision) {
        return Ok(None);
    }
    if workspace {
        if let Some(view) = runtime.inspect_run(db, engine, run)? {
            return Ok(Some(view));
        }
    } else {
        let tx = db.unchecked_transaction()?;
        if let Some(view) = conversations.persisted_inspection(&tx, engine, run)? {
            return Ok(Some(view));
        }
    }
    let tx = db.unchecked_transaction()?;
    let mut reader = graph_store::BorrowedReadStore::new(&tx, partition).map_err(graph_error)?;
    let checkpoint = checkpoint(&reader)?;
    let history = checkpoint
        .historical_inspection(checkpoint.stamp().revision, limits(), &mut reader)
        .map_err(graph_error)?;
    history
        .snapshot(run, export())
        .map(Some)
        .map_err(graph_error)
}

#[cfg(test)]
pub(crate) fn verify_workspace_reads(store: &crate::storage::store::Store) {
    let db = &store.connection;
    let changes = db.total_changes();
    let page = workspace_runs(db, None).unwrap();
    assert!(!page.runs.is_empty());
    for entry in page.runs {
        let current = current(
            db,
            &store.workspace_graphs,
            &store.graph_runtime,
            &entry.engine,
            &entry.run,
            None,
        )
        .unwrap()
        .unwrap();
        assert_eq!(current.artifact_id, entry.artifact);
        assert!(
            self::current(
                db,
                &store.workspace_graphs,
                &store.graph_runtime,
                &entry.engine,
                &entry.run,
                Some(&current.revision)
            )
            .unwrap()
            .is_none()
        );
        let history = history(db, &entry.engine, &entry.run, None).unwrap();
        assert!(!history.frames.is_empty());
        for (node, attempts) in &current.attempts {
            for selected in attempts {
                let detail = attempt(
                    db,
                    &entry.engine,
                    &entry.run,
                    &current.revision,
                    node,
                    &selected.id,
                )
                .unwrap();
                assert_eq!(detail.execution, selected.execution);
            }
        }
    }
    assert_eq!(db.total_changes(), changes);
}
