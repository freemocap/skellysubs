//! Native receipts retain domain results; native state decides whether a
//! receipt is current. Live reads use verified records; retained-only reads replay
//! history. There is no second SQL implementation of attempt state.
use super::*;
use crate::ai::{graph, graph_store};
use crate::conversations::execution::graph_runtime;

struct CurrentReceipt {
    id: String,
    raw: String,
    node: String,
    attempt: String,
    run: String,
    conversation: String,
    catalog: String,
    engine: String,
}

pub(super) fn current(db: &Connection, turn: &str, kind: &str) -> Result<Option<(String, Value)>> {
    current_using(db, turn, kind, None)
}

pub(super) fn current_using(
    db: &Connection,
    turn: &str,
    kind: &str,
    runtime: Option<&graph_runtime::Runtime>,
) -> Result<Option<(String, Value)>> {
    if db.is_autocommit() {
        let tx = db.unchecked_transaction()?;
        return current_using(&tx, turn, kind, runtime);
    }
    let saved: Option<CurrentReceipt> = db.query_row(
        "SELECT a.id,a.result,a.node_key,a.attempt_id,a.run_id,t.conversation_id,e.catalog,a.engine_id FROM conversation_graph_assessments a JOIN turns t ON t.id=a.turn_id JOIN graph_engines e ON e.id=a.engine_id WHERE a.turn_id=?1 AND a.kind=?2 AND t.state NOT IN ('cancelled','invalidated') AND (?2!='coach_feedback' OR a.run_id=COALESCE((SELECT h.run_id FROM graph_helper_requests h WHERE h.turn_id=a.turn_id AND h.operation=?3 ORDER BY h.rowid DESC LIMIT 1),(SELECT o.run_id FROM turn_execution_owners o WHERE o.turn_id=a.turn_id))) ORDER BY a.rowid DESC LIMIT 1",
        params![turn,kind,serde_json::to_string(&crate::learning::coaching::feedback_graph::operation_contract())?],|r| Ok(CurrentReceipt { id:r.get(0)?,raw:r.get(1)?,node:r.get(2)?,attempt:r.get(3)?,run:r.get(4)?,conversation:r.get(5)?,catalog:r.get(6)?,engine:r.get(7)? })).optional()?;
    let Some(CurrentReceipt {
        id,
        raw,
        node,
        attempt,
        run,
        conversation,
        catalog,
        engine,
    }) = saved
    else {
        return Ok(None);
    };
    if let Some(current) = runtime
        .map(|runtime| runtime.persisted_attempt_is_current(db, &engine, &run, &node, &attempt))
        .transpose()?
        .flatten()
    {
        return if current {
            Ok(Some((id, serde_json::from_str(&raw)?)))
        } else {
            Ok(None)
        };
    }
    let snapshot = {
        let limits = graph_runtime::limits();
        let mut reader = graph_store::BorrowedReadStore::new(
            db,
            graph_store::Partition {
                owner: crate::ai::graph_store::Owner::Conversation(conversation),
                catalog,
            },
        )
        .map_err(graph_runtime::error)?;
        let checkpoint = reader
            .checkpoint(limits.checkpoint)
            .map_err(graph_runtime::error)?
            .ok_or_else(|| {
                AppError::new(
                    ErrorCode::Storage,
                    "Assessment graph checkpoint is missing.",
                )
            })?;
        checkpoint
            .historical_inspection(
                checkpoint.stamp().revision,
                graph::HistoricalLimits {
                    history: limits.history,
                    state: limits.state,
                },
                &mut reader,
            )
            .map_err(graph_runtime::error)?
            .snapshot(
                &run,
                graph::ExportLimits {
                    bytes: 4 * 1024 * 1024,
                    attempts: 4096,
                },
            )
            .map_err(graph_runtime::error)?
    };
    if snapshot.nodes.get(&node) != Some(&graph::Disposition::Adopted)
        || !snapshot
            .attempts
            .get(&node)
            .and_then(|a| a.last())
            .is_some_and(|a| a.id == attempt)
    {
        return Ok(None);
    }
    Ok(Some((id, serde_json::from_str(&raw)?)))
}

pub(super) fn disclosure(db: &Connection, id: &str) -> Result<Option<String>> {
    Ok(db
        .query_row(
            "SELECT decision FROM conversation_graph_disclosures WHERE assessment_id=?1",
            [id],
            |r| r.get(0),
        )
        .optional()?)
}
pub(super) fn disclose(db: &Connection, id: &str, value: &Value) -> Result<()> {
    db.execute("INSERT INTO conversation_graph_disclosures(assessment_id,decision) VALUES(?1,?2) ON CONFLICT(assessment_id) DO UPDATE SET decision=excluded.decision",params![id,value.to_string()])?;
    Ok(())
}
