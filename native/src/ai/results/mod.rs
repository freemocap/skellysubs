//! Workspace-wide request results and evictable blobs. No product owner or UI state.
use crate::model::{AppError, ErrorCode, Result};
use rusqlite::{Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use ts_rs::TS;

pub mod pending;
pub mod speech;
pub mod transcription;

#[derive(Clone)]
pub struct Retained {
    pub cached: bool,
    pub execution: String,
    pub payload: Vec<u8>,
    pub metadata: serde_json::Value,
}

#[derive(Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct CacheSettings {
    #[ts(type = "number")]
    pub capacity_bytes: u64,
    #[ts(type = "number")]
    pub used_bytes: u64,
    #[ts(type = "number")]
    pub result_count: u64,
}

pub fn digest(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

#[cfg(test)]
pub fn initialize(db: &Connection) -> Result<()> {
    crate::speech::analysis::signal_cache::initialize(db)?;
    let schema = include_str!("../../storage/schemas/inference_results.sql");
    let expected = Connection::open_in_memory()?;
    expected.execute_batch(schema)?;
    let mut objects =
        expected.prepare("SELECT name,sql FROM sqlite_master WHERE sql IS NOT NULL")?;
    for object in objects.query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)))? {
        let (name, sql) = object?;
        let actual: Option<String> = db
            .query_row(
                "SELECT sql FROM sqlite_master WHERE name=?1",
                [&name],
                |r| r.get(0),
            )
            .optional()?;
        if actual.is_some_and(|actual| actual != sql) {
            return Err(AppError::new(
                ErrorCode::Storage,
                format!(
                    "Unexpected inference schema object: {name}. No inference data was changed."
                ),
            ));
        }
    }
    db.execute_batch(schema)?;
    if !db.query_row(
        "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE name='graph_audio_receipts')",
        [],
        |r| r.get::<_, bool>(0),
    )? {
        db.execute_batch(include_str!("../../storage/schemas/graph_audio.sql"))?;
    }
    if !db.query_row(
        "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE name='workspace_reading_cache')",
        [],
        |r| r.get::<_, bool>(0),
    )? {
        db.execute_batch(include_str!(
            "../../storage/schemas/workspace_reading_cache.sql"
        ))?;
    }
    if !db.query_row(
        "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE name='workspace_graph_audio_receipts')",
        [],
        |r| r.get::<_, bool>(0),
    )? {
        db.execute_batch(include_str!(
            "../../storage/schemas/workspace_graph_audio.sql"
        ))?;
    }
    if !db.query_row(
        "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE name='workspace_transcription_cache')",
        [],
        |r| r.get::<_, bool>(0),
    )? {
        db.execute_batch(include_str!(
            "../../storage/schemas/workspace_transcription_cache.sql"
        ))?;
    }
    db.execute_batch(include_str!("../../storage/schemas/ai_content.sql"))?;
    db.execute_batch(include_str!(
        "../../storage/schemas/inference_blob_references.sql"
    ))?;
    recover(db)
}

pub(crate) fn recover(db: &Connection) -> Result<()> {
    crate::speech::analysis::signal_cache::recover(db)?;
    let transaction = db.unchecked_transaction()?;
    transaction.execute(
        "UPDATE ai_content_records SET state='interrupted' WHERE state='pending'",
        [],
    )?;
    prune(&transaction)?;
    transaction.commit()?;
    Ok(())
}

pub(crate) fn tick(db: &Connection) -> Result<i64> {
    Ok(db.query_row(
        "UPDATE inference_cache_settings SET clock=clock+1 WHERE singleton=1 RETURNING clock",
        [],
        |r| r.get(0),
    )?)
}

pub fn settings(db: &Connection) -> Result<CacheSettings> {
    let (capacity_bytes, used_bytes) = byte_usage(db)?;
    Ok(CacheSettings {
        capacity_bytes,
        used_bytes,
        // Reusable result count excludes private originals.
        result_count: db.query_row("SELECT (SELECT count(*) FROM native_graph_audio_cache)+(SELECT count(*) FROM workspace_reading_cache)+(SELECT count(*) FROM workspace_transcription_cache)", [], |r| {
            r.get::<_, i64>(0)
        })? as u64,
    })
}

fn byte_usage(db: &Connection) -> Result<(u64, u64)> {
    Ok((
        db.query_row(
            "SELECT capacity_bytes FROM inference_cache_settings WHERE singleton=1",
            [],
            |r| r.get::<_, i64>(0),
        )? as u64,
        (db.query_row(
            "SELECT COALESCE(SUM(length(payload)),0) FROM inference_blobs",
            [],
            |r| r.get::<_, i64>(0),
        )? + crate::speech::analysis::signal_cache::bytes(db, "inference")?) as u64,
    ))
}

/// Add-only writes need eviction only when the byte budget is exceeded. Startup,
/// owner cleanup and capacity changes still use full reachability maintenance.
pub(crate) fn enforce_capacity(db: &Connection) -> Result<()> {
    let (capacity, used) = byte_usage(db)?;
    if used > capacity {
        prune(db)?;
    }
    Ok(())
}

pub fn set_capacity(db: &Connection, bytes: u64) -> Result<CacheSettings> {
    if bytes > 100_000 * 1024 * 1024 {
        return Err(AppError::new(
            ErrorCode::Validation,
            "Cache capacity is too large.",
        ));
    }
    let tx = db.unchecked_transaction()?;
    tx.execute(
        "UPDATE inference_cache_settings SET capacity_bytes=?1 WHERE singleton=1",
        [bytes as i64],
    )?;
    prune(&tx)?;
    tx.commit()?;
    settings(db)
}

pub(crate) fn prune(db: &Connection) -> Result<()> {
    loop {
        db.execute("DELETE FROM inference_blobs WHERE NOT EXISTS(SELECT 1 FROM native_graph_audio_cache WHERE blob_digest=inference_blobs.digest) AND NOT EXISTS(SELECT 1 FROM workspace_reading_cache WHERE blob_digest=inference_blobs.digest) AND NOT EXISTS(SELECT 1 FROM workspace_transcription_cache WHERE blob_digest=inference_blobs.digest) AND NOT EXISTS(SELECT 1 FROM ai_content_chunks WHERE blob_digest=inference_blobs.digest)", [])?;
        let removed = db.prepare("SELECT id FROM audio_signal_sources WHERE kind='inference' AND NOT EXISTS(SELECT 1 FROM native_graph_audio_cache g WHERE g.receipt_id=audio_signal_sources.id)")?
            .query_map([], |r| r.get::<_, String>(0))?.collect::<rusqlite::Result<Vec<_>>>()?;
        for id in removed {
            crate::speech::analysis::signal_cache::release(db, "inference", &id)?;
        }
        let (capacity, used) = byte_usage(db)?;
        if used <= capacity {
            return Ok(());
        }
        let (native, id): (i32, String) = db.query_row("SELECT native,id FROM (SELECT 1 AS native,receipt_id AS id,last_used FROM graph_audio_cache UNION ALL SELECT 2 AS native,run_id AS id,last_used FROM workspace_reading_cache UNION ALL SELECT 3 AS native,receipt_id AS id,last_used FROM workspace_graph_audio_cache UNION ALL SELECT 4 AS native,run_id AS id,last_used FROM workspace_transcription_cache UNION ALL SELECT 5 AS native,id,last_used FROM ai_content_records WHERE EXISTS(SELECT 1 FROM ai_content_chunks WHERE record_id=ai_content_records.id)) ORDER BY last_used,id,native LIMIT 1", [], |r| Ok((r.get(0)?,r.get(1)?)))?;
        if native == 5 {
            db.execute(
                "UPDATE ai_content_records SET availability='evicted' WHERE id=?1",
                [&id],
            )?;
            db.execute("DELETE FROM ai_content_chunks WHERE record_id=?1", [&id])?;
            continue;
        }
        db.execute(
            if native == 4 {
                "DELETE FROM workspace_transcription_cache WHERE run_id=?1"
            } else if native == 3 {
                "DELETE FROM workspace_graph_audio_cache WHERE receipt_id=?1"
            } else if native == 2 {
                "DELETE FROM workspace_reading_cache WHERE run_id=?1"
            } else {
                "DELETE FROM graph_audio_cache WHERE receipt_id=?1"
            },
            [id],
        )?;
    }
}

#[cfg(test)]
pub fn for_consumer(db: &Connection, consumer: &str) -> Result<Option<Retained>> {
    if native_consumer(db, consumer)?.is_some() {
        let stream: Option<String> = db.query_row(
            "SELECT stream_id FROM workspace_graph_consumers WHERE consumer_id=?1",
            [consumer],
            |r| r.get(0),
        )?;
        let Some(stream) = stream else {
            return Ok(None);
        };
        let tx = db.unchecked_transaction()?;
        let audio = crate::speech::graph_audio::read(&tx, &stream)?;
        tx.commit()?;
        return audio
            .map(|audio| {
                Ok(Retained {
                    cached: true,
                    execution: audio.receipt.id,
                    payload: serde_json::to_vec(&crate::speech::alignment::SpeechAudio::new(
                        &audio.wav,
                        audio.alignment,
                    ))?,
                    metadata: receipt_for_consumer(db, consumer)?.unwrap_or_default()["response"]
                        .clone(),
                })
            })
            .transpose();
    }
    Ok(None)
}

/// Receipts remain inspectable after payload eviction or consumer cancellation.
pub fn receipt_for_consumer(db: &Connection, consumer: &str) -> Result<Option<serde_json::Value>> {
    if let Some(run) = native_consumer(db, consumer)? {
        let source: Option<(String,String)> = db.query_row("SELECT a.engine_id,a.execution_id FROM workspace_graph_consumers c JOIN native_graph_audio_receipts a ON a.id=c.stream_id WHERE c.consumer_id=?1",[consumer],|r|Ok((r.get(0)?,r.get(1)?))).optional()?;
        if let Some((engine, execution)) = source {
            let mut receipt = crate::ai::workspace_graph::execution_receipt(
                db,
                &engine,
                serde_json::from_str(&execution)?,
            )?;
            receipt["nativeRun"] = serde_json::json!(run);
            return Ok(Some(receipt));
        }
        return crate::ai::workspace_graph::receipt(db, &run);
    }
    Ok(None)
}

#[cfg(test)]
mod tests;

fn native_consumer(db: &Connection, consumer: &str) -> Result<Option<String>> {
    Ok(db
        .query_row(
            "SELECT run_id FROM workspace_graph_consumers WHERE consumer_id=?1",
            [consumer],
            |r| r.get(0),
        )
        .optional()?)
}
