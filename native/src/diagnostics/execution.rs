//! Export the same retained facts used by AI activity, never protected graph
//! inputs, outputs or provisional audio/text. This is a read-only snapshot.
use crate::{
    ai::{graph, workspace_graph},
    model::{AppError, ErrorCode, Result},
};
use rusqlite::Connection;
use serde_json::json;
use std::{collections::BTreeSet, fs, io::Write, path::Path};

fn failure() -> AppError {
    AppError::new(
        ErrorCode::Storage,
        "Could not export execution diagnostics.",
    )
}
fn io(cause: std::io::Error) -> AppError {
    super::response::io_context(&cause, "execution_diagnostics", failure())
}
fn line(out: &mut impl Write, value: &impl serde::Serialize) -> Result<()> {
    serde_json::to_writer(&mut *out, value)?;
    out.write_all(b"\n").map_err(io)
}

/// Publish only a completely written snapshot. Replacement removes stale
/// workspace evidence, including after reset or a failed workspace startup.
pub(super) fn save(db: Option<&Connection>, root: &Path) -> Result<()> {
    let temporary = root.join(format!(".execution-{}.tmp", uuid::Uuid::new_v4()));
    let mut options = fs::OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let mut file = options.open(&temporary).map_err(io)?;
    let result = (|| {
        write(db, &mut file)?;
        file.sync_all().map_err(io)?;
        fs::rename(&temporary, root.join("execution-diagnostics.jsonl")).map_err(io)
    })();
    if let Err(mut error) = result {
        if let Err(cleanup) = fs::remove_file(&temporary) {
            error.diagnostics.get_or_insert_with(|| json!({}))["cleanup_error"] =
                super::response::error_metadata(&io(cleanup), &[]);
        }
        return Err(error);
    }
    Ok(())
}

fn write(db: Option<&Connection>, out: &mut impl Write) -> Result<()> {
    line(
        out,
        &json!({"code":"execution_snapshot", "formatVersion":1,
        "createdAtMs":super::timestamp()?, "workspaceAvailable":db.is_some(),
        "scope":"All retained conversation and workspace graph runs and attempts",
        "omitted":"Inputs, outputs, provisional content, recordings and credentials"}),
    )?;
    let Some(db) = db else {
        return line(
            out,
            &json!({"code":"execution_snapshot_complete", "workspaceAvailable":false, "engines":0}),
        );
    };
    let engines = db.prepare("SELECT id FROM graph_engines UNION ALL SELECT id FROM workspace_graph_engines ORDER BY id")?
        .query_map([], |row| row.get::<_, String>(0))?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    for engine in &engines {
        let history = workspace_graph::history::inspection(db, engine)?;
        write_history(&history, out)?;
    }
    line(
        out,
        &json!({"code":"execution_snapshot_complete", "workspaceAvailable":true, "engines":engines.len()}),
    )
}

/// Pagination includes older attempts/retries. Shared execution evidence is
/// written once per engine; every consumer retains its run/node/attempt link.
pub(crate) fn write_history(
    history: &graph::HistoricalInspection,
    out: &mut impl Write,
) -> Result<()> {
    let mut seen = BTreeSet::new();
    for run in history.run_ids() {
        let mut cursor = None;
        loop {
            let page = history
                .page(
                    run,
                    cursor.as_ref(),
                    graph::ExportLimits {
                        bytes: 32 * 1024 * 1024,
                        attempts: 256,
                    },
                )
                .map_err(workspace_graph::error)?;
            // Native projection already omits constants and unclassified faults.
            // Do not serialize execution_content or the underlying checkpoint.
            line(out, &json!({"code":"execution_run", "snapshot":page}))?;
            for record in &page.attempts.records {
                if !seen.insert(record.attempt.execution.clone()) {
                    continue;
                }
                let execution = serde_json::from_str(&record.attempt.execution)?;
                let evidence = history
                    .execution_evidence(execution)
                    .map_err(workspace_graph::error)?;
                let receipt = history
                    .producer_receipt(execution)
                    .map_err(workspace_graph::error)?;
                let state = match receipt.outcome {
                    Some(Ok(())) => "succeeded",
                    Some(Err(_)) => "failed",
                    None if receipt.unknown => "unknown",
                    None => "pending",
                };
                line(
                    out,
                    &json!({"code":"execution_evidence", "engine":page.engine,
                    "execution":record.attempt.execution, "state":state,
                    "dispatched":receipt.dispatched, "evidenceAvailable":evidence.is_some(),
                    "complete":evidence.as_ref().map(|e|e.complete),
                    "captureFailed":evidence.as_ref().map(|e|e.evidence_failure.is_some()),
                    "captureFailureDetails":evidence.as_ref().and_then(|e|e.evidence_failure.as_ref()).map(|_|"[omitted: unclassified adapter fault]")}),
                )?;
                if let Some(evidence) = evidence {
                    // ResponseEvidence is the adapter-classified metadata contract.
                    // Keep every observation (including partial success before failure).
                    // Never serialize ExecutionEvidence: provisional holds private content.
                    for (index, observation) in evidence.observations.iter().enumerate() {
                        line(
                            out,
                            &json!({"code":"execution_response", "engine":page.engine,
                            "execution":record.attempt.execution, "observation":index,
                            "response":observation}),
                        )?;
                    }
                }
            }
            cursor = page.attempts.next;
            if cursor.is_none() {
                break;
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn unavailable_workspace_replaces_previous_snapshot_explicitly() {
        let dir = tempfile::tempdir().unwrap();
        let target = dir.path().join("execution-diagnostics.jsonl");
        fs::write(&target, "previous-workspace-canary").unwrap();
        save(None, dir.path()).unwrap();
        let text = fs::read_to_string(target).unwrap();
        assert!(!text.contains("previous-workspace-canary"));
        assert!(text.contains("\"workspaceAvailable\":false"));
        assert!(text.contains("execution_snapshot_complete"));
    }
    #[test]
    fn database_read_failure_does_not_publish_partial_snapshot() {
        let dir = tempfile::tempdir().unwrap();
        let db = Connection::open_in_memory().unwrap();
        assert!(save(Some(&db), dir.path()).is_err());
        assert_eq!(fs::read_dir(dir.path()).unwrap().count(), 0);
    }
    #[test]
    fn empty_workspace_is_a_complete_snapshot() {
        let dir = tempfile::tempdir().unwrap();
        let store = crate::storage::store::Store::open(&dir.path().join("workspace")).unwrap();
        save(Some(&store.connection), dir.path()).unwrap();
        let text = fs::read_to_string(dir.path().join("execution-diagnostics.jsonl")).unwrap();
        assert!(text.contains("\"workspaceAvailable\":true"));
        assert!(text.contains("execution_snapshot_complete"));
    }
}
