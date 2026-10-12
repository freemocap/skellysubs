//! Bounded reading projections share the blob LRU and native execution ownership.
use super::*;
use crate::ai::results;
use rusqlite::{Connection, OptionalExtension, params};

pub fn scope(db: &Connection, source: &str, fresh: bool, run: &str) -> Result<String> {
    if fresh {
        return Ok(run.into());
    }
    Ok(db.query_row("SELECT json_extract(r.context,'$.reuseScope') FROM workspace_graph_runs r JOIN reading_attempts a ON a.id=r.run_id WHERE json_extract(r.context,'$.sourceKey')=?1 AND (EXISTS(SELECT 1 FROM workspace_reading_cache c WHERE c.run_id=r.run_id) OR json_extract(a.receipt,'$.state') IN ('pending','running')) ORDER BY r.rowid DESC LIMIT 1", [source], |r| r.get(0)).optional()?.unwrap_or_else(|| run.into()))
}
pub fn previous(db: &Connection, source: &str) -> Result<Option<text::Stored>> {
    let payload: Option<(Vec<u8>,String)> = db.query_row("SELECT b.payload,b.digest FROM workspace_reading_cache c JOIN inference_blobs b ON b.digest=c.blob_digest WHERE c.source_key=?1 ORDER BY c.last_used DESC LIMIT 1", [source], |r| Ok((r.get(0)?,r.get(1)?))).optional()?;
    payload
        .map(|(bytes, expected)| {
            if results::digest(&bytes) != expected {
                return Err(AppError::new(
                    ErrorCode::Storage,
                    "Cached reading result failed its integrity check.",
                ));
            }
            text::Stored::decode(&bytes)
        })
        .transpose()
}
pub fn retain(db: &Connection, run: &str, source: &str, stored: &text::Stored) -> Result<()> {
    let bytes = serde_json::to_vec(stored)?;
    let tx = db.unchecked_transaction()?;
    if bytes.len() as u64 <= results::settings(&tx)?.capacity_bytes {
        let digest = results::digest(&bytes);
        tx.execute(
            "INSERT OR IGNORE INTO inference_blobs(digest,payload) VALUES(?1,?2)",
            params![digest, bytes],
        )?;
        tx.execute("INSERT INTO workspace_reading_cache(run_id,source_key,blob_digest,last_used) VALUES(?1,?2,?3,?4) ON CONFLICT(run_id) DO UPDATE SET blob_digest=excluded.blob_digest,last_used=excluded.last_used", params![run,source,digest,results::tick(&tx)?])?;
        results::prune(&tx)?;
    }
    tx.commit()?;
    Ok(())
}
pub fn sources(
    db: &Connection,
    query: &saved::SavedGlossQuery,
    context: &crate::configuration::LanguageContext,
    access: &str,
) -> Result<Vec<saved::SavedGlossSource>> {
    let rows = db.prepare("SELECT c.run_id,b.payload,b.digest FROM workspace_reading_cache c JOIN inference_blobs b ON b.digest=c.blob_digest WHERE json_extract(CAST(b.payload AS TEXT),'$.access_scope')=?1 AND json_extract(CAST(b.payload AS TEXT),'$.scope.language')=?2 AND json_extract(CAST(b.payload AS TEXT),'$.scope.variety')=?3 AND json_extract(CAST(b.payload AS TEXT),'$.scope.explanation')=?4 AND json_extract(CAST(b.payload AS TEXT),'$.scope.explanationVariety')=?5 AND json_type(CAST(b.payload AS TEXT),'$.gloss')='object' AND EXISTS(SELECT 1 FROM json_each(?6) s WHERE instr(json_extract(CAST(b.payload AS TEXT),'$.text'),s.value)>0) ORDER BY c.last_used")?
        .query_map(params![access,context.language_id,context.variety_id,context.explanation_language_id,context.explanation_variety_id,serde_json::to_string(&query.surfaces)?], |r| Ok((r.get::<_,String>(0)?,r.get::<_,Vec<u8>>(1)?,r.get::<_,String>(2)?)))?.collect::<rusqlite::Result<Vec<_>>>()?;
    rows.into_iter()
        .map(|(run, bytes, digest)| {
            if results::digest(&bytes) != digest {
                return Err(AppError::new(
                    ErrorCode::Storage,
                    "Cached reading result failed its integrity check.",
                ));
            }
            let stored = text::Stored::decode(&bytes)?;
            let view = stored
                .gloss
                .ok_or_else(|| AppError::new(ErrorCode::Storage, "Cached gloss is absent."))?;
            Ok(saved::SavedGlossSource {
                dictionary: None,
                provenance: None,
                source_id: format!("graph-reading/{run}"),
                operation_id: Some(view.operation_id),
                attempt_id: Some(view.attempt_id),
                scope: stored.scope,
                text: stored.text,
                segments: view.segments,
            })
        })
        .collect()
}
