//! Evictable guide projections reference durable native execution receipts.
use super::*;
use rusqlite::{Connection, OptionalExtension, params};
pub(super) fn receipt(db: &Connection, run: &str) -> Result<serde_json::Value> {
    crate::ai::workspace_graph::receipt(db, run)?
        .ok_or_else(|| AppError::new(ErrorCode::Storage, "Guide execution receipt is missing."))
}
pub(super) fn retained(receipt: serde_json::Value, payload: Vec<u8>, cached: bool) -> Retained {
    Retained {
        cached,
        execution: format!(
            "graph:{}:{}",
            receipt["nativeEngine"].as_str().unwrap_or_default(),
            receipt["nativeExecution"]
        ),
        payload,
        metadata: receipt["response"].clone(),
    }
}
pub(super) fn lookup(db: &Connection, key: &str) -> Result<Option<Retained>> {
    let saved: Option<(String,Vec<u8>,String)> = db.query_row("SELECT c.run_id,b.payload,b.digest FROM workspace_reading_cache c JOIN workspace_graph_runs r ON r.run_id=c.run_id JOIN inference_blobs b ON b.digest=c.blob_digest WHERE c.source_key=?1 AND r.kind='guide_translation' ORDER BY c.last_used DESC LIMIT 1",[key],|r| Ok((r.get(0)?,r.get(1)?,r.get(2)?))).optional()?;
    saved
        .map(|(run, payload, digest)| {
            if results::digest(&payload) != digest {
                return Err(AppError::new(
                    ErrorCode::Storage,
                    "Cached guide failed its integrity check.",
                ));
            }
            let receipt = receipt(db, &run)?;
            db.execute(
                "UPDATE workspace_reading_cache SET last_used=?2 WHERE run_id=?1",
                params![run, results::tick(db)?],
            )?;
            Ok(retained(receipt, payload, true))
        })
        .transpose()
}
pub(super) fn check_retry(
    db: &Connection,
    key: &str,
    active: impl FnOnce(&str) -> bool,
) -> Result<()> {
    let run: Option<String> = db.query_row("SELECT run_id FROM workspace_graph_runs WHERE kind='guide_translation' AND json_extract(context,'$.guideKey')=?1 ORDER BY rowid DESC LIMIT 1",[key],|r| r.get(0)).optional()?;
    if let Some(run) = run {
        let receipt = receipt(db, &run)?;
        if matches!(
            receipt["state"].as_str(),
            Some("failed" | "unknown" | "cancelled" | "held")
        ) || (receipt["state"] == "pending" && !active(&run))
        {
            return Err(AppError::new(
                ErrorCode::Provider,
                "Guide translation needs an explicit retry.",
            )
            .with_diagnostics(json!({"sourceExecution":receipt})));
        }
    }
    Ok(())
}
pub(super) fn retain(db: &Connection, run: &str, key: &str, payload: &[u8]) -> Result<()> {
    let tx = db.unchecked_transaction()?;
    if payload.len() as u64 <= results::settings(&tx)?.capacity_bytes {
        let digest = results::digest(payload);
        tx.execute(
            "INSERT OR IGNORE INTO inference_blobs(digest,payload) VALUES(?1,?2)",
            params![digest, payload],
        )?;
        tx.execute("INSERT INTO workspace_reading_cache(run_id,source_key,blob_digest,last_used) VALUES(?1,?2,?3,?4)",params![run,key,digest,results::tick(&tx)?])?;
        results::prune(&tx)?;
    }
    tx.commit()?;
    Ok(())
}
