//! Private originals, separate from graph facts and diagnostic projections.
use crate::model::{AppError, ErrorCode, Result};
use rusqlite::{Connection, OptionalExtension, params};
use serde::{Deserialize, Serialize};
use ts_rs::TS;

pub mod capture;
#[cfg(test)]
pub(crate) mod tests;

pub enum Owner {
    Execution { engine: String, execution: String },
    Recording(String),
}
#[derive(Clone, Serialize, Deserialize)]
#[serde(tag = "event", rename_all = "snake_case")]
pub enum Write {
    Begin {
        id: String,
        kind: String,
        media_type: String,
    },
    Append {
        id: String,
        #[serde(with = "base64_bytes")]
        bytes: Vec<u8>,
    },
    Finish {
        id: String,
        complete: bool,
        digest: String,
        credentials_removed: bool,
    },
}

#[derive(Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct ContentRecord {
    pub id: String,
    pub kind: String,
    pub media_type: String,
    pub state: String,
    pub availability: String,
    pub byte_count: u32,
    pub digest: Option<String>,
    pub credentials_removed: bool,
}
#[derive(Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct ContentPayload {
    pub record: ContentRecord,
    pub text: Option<String>,
    pub base64: Option<String>,
}
fn invalid() -> AppError {
    AppError::new(
        ErrorCode::Storage,
        "Original content could not be retained or verified.",
    )
}

fn owner_exists(db: &Connection, owner: &Owner) -> Result<bool> {
    let exists:bool=match owner {
        Owner::Execution {engine,..}=>db.query_row("SELECT EXISTS(SELECT 1 FROM graph_engines WHERE id=?1 UNION ALL SELECT 1 FROM workspace_graph_engines WHERE id=?1)",[engine],|r|r.get(0))?,
        Owner::Recording(id)=>db.query_row("SELECT EXISTS(SELECT 1 FROM transcription_attempts WHERE id=?1)",[id],|r|r.get(0))?,
    };
    Ok(exists)
}

pub fn write(db: &Connection, owner: &Owner, event: Write) -> Result<()> {
    let tx = db.unchecked_transaction()?;
    let exists = owner_exists(&tx, owner)?;
    if !exists {
        return Err(AppError::new(
            ErrorCode::Conflict,
            "Original content owner was removed.",
        ));
    }
    match event {
        Write::Begin { id, kind, media_type } => {
            let (conversation, workspace, recording, execution) = match owner {
                Owner::Execution { engine, execution } => {
                    let conversation: bool = tx.query_row("SELECT EXISTS(SELECT 1 FROM graph_engines WHERE id=?1)", [engine], |r| r.get(0))?;
                    let workspace: bool = tx.query_row("SELECT EXISTS(SELECT 1 FROM workspace_graph_engines WHERE id=?1)", [engine], |r| r.get(0))?;
                    if conversation == workspace { return Err(invalid()); }
                    (conversation.then_some(engine), workspace.then_some(engine), None, Some(execution))
                }
                Owner::Recording(id) => (None, None, Some(id), None),
            };
            tx.execute("INSERT INTO ai_content_records(id,conversation_engine,workspace_engine,recording_id,execution_id,kind,media_type,state,availability,last_used) VALUES(?1,?2,?3,?4,?5,?6,?7,'pending','retained',?8)", params![id,conversation,workspace,recording,execution,kind,media_type,super::results::tick(&tx)?])?;
        }
        Write::Append { id, bytes } => {
            let (state, availability, size): (String,String,u32) = tx.query_row("SELECT state,availability,byte_count FROM ai_content_records WHERE id=?1", [&id], |r| Ok((r.get(0)?,r.get(1)?,r.get(2)?)))?;
            if state != "pending" || size as usize + bytes.len() > 64 * 1024 * 1024 { return Err(invalid()); }
            if !bytes.is_empty() && availability == "retained" {
                let digest = super::results::digest(&bytes);
                tx.execute("INSERT OR IGNORE INTO inference_blobs(digest,payload) VALUES(?1,?2)", params![digest,bytes])?;
                tx.execute("INSERT INTO ai_content_chunks(record_id,sequence,blob_digest) SELECT ?1,COALESCE(MAX(sequence)+1,0),?2 FROM ai_content_chunks WHERE record_id=?1", params![id,digest])?;
            }
            tx.execute("UPDATE ai_content_records SET byte_count=byte_count+?2,last_used=?3 WHERE id=?1", params![id,bytes.len() as i64,super::results::tick(&tx)?])?;
        }
        Write::Finish { id, complete, digest, credentials_removed } => {
            if tx.execute("UPDATE ai_content_records SET state=?2,digest=?3,credentials_removed=?4 WHERE id=?1 AND state='pending'", params![id,if complete {"complete"} else {"partial"},digest,credentials_removed])? != 1 { return Err(invalid()); }
        }
    }
    super::results::prune(&tx)?;
    tx.commit()?;
    Ok(())
}

fn row(r: &rusqlite::Row<'_>) -> rusqlite::Result<ContentRecord> {
    Ok(ContentRecord {
        id: r.get(0)?,
        kind: r.get(1)?,
        media_type: r.get(2)?,
        state: r.get(3)?,
        availability: r.get(4)?,
        byte_count: r.get(5)?,
        digest: r.get(6)?,
        credentials_removed: r.get(7)?,
    })
}
const FIELDS: &str = "id,kind,media_type,state,availability,byte_count,digest,credentials_removed";
pub fn for_execution(db: &Connection, engine: &str, execution: &str) -> Result<Vec<ContentRecord>> {
    Ok(db.prepare(&format!("SELECT {FIELDS} FROM ai_content_records WHERE (conversation_engine=?1 OR workspace_engine=?1) AND execution_id=?2 ORDER BY rowid"))?.query_map(params![engine,execution],row)?.collect::<rusqlite::Result<_>>()?)
}
pub struct PreparedContent {
    record: ContentRecord,
    chunks: Vec<(Vec<u8>, String)>,
}
pub fn prepare(db: &Connection, id: &str) -> Result<PreparedContent> {
    let tx = db.unchecked_transaction()?;
    let record = tx
        .query_row(
            &format!("SELECT {FIELDS} FROM ai_content_records WHERE id=?1"),
            [id],
            row,
        )
        .optional()?
        .ok_or_else(invalid)?;
    if record.byte_count as usize > 64 * 1024 * 1024 {
        return Err(invalid());
    }
    let chunks = if record.availability == "retained" {
        let (bytes,count):(i64,i64)=tx.query_row("SELECT COALESCE(SUM(length(b.payload)),0),count(*) FROM ai_content_chunks c JOIN inference_blobs b ON b.digest=c.blob_digest WHERE c.record_id=?1",[id],|r|Ok((r.get(0)?,r.get(1)?)))?;
        if bytes != i64::from(record.byte_count) || count > 100_000 {
            return Err(invalid());
        }

        let mut query=tx.prepare("SELECT b.payload,c.blob_digest FROM ai_content_chunks c JOIN inference_blobs b ON b.digest=c.blob_digest WHERE c.record_id=?1 ORDER BY c.sequence")?;
        let chunks = query
            .query_map([id], |r| {
                Ok((r.get::<_, Vec<u8>>(0)?, r.get::<_, String>(1)?))
            })?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        tx.execute(
            "UPDATE ai_content_records SET last_used=?2 WHERE id=?1",
            params![id, super::results::tick(&tx)?],
        )?;
        chunks
    } else {
        Vec::new()
    };
    tx.commit()?;
    Ok(PreparedContent { record, chunks })
}
impl PreparedContent {
    /// Verification and text/base64 conversion run after releasing the store lock.
    pub fn finish(self) -> Result<ContentPayload> {
        use base64::Engine;
        let Self { record, chunks } = self;
        if record.availability != "retained" {
            return Ok(ContentPayload {
                record,
                text: None,
                base64: None,
            });
        }
        let mut bytes = Vec::with_capacity(record.byte_count as usize);
        for (chunk, digest) in chunks {
            if super::results::digest(&chunk) != digest
                || bytes.len() + chunk.len() > 64 * 1024 * 1024
            {
                return Err(invalid());
            }
            bytes.extend_from_slice(&chunk);
        }
        if bytes.len() != record.byte_count as usize
            || record
                .digest
                .as_ref()
                .is_some_and(|d| super::results::digest(&bytes) != *d)
        {
            return Err(invalid());
        }
        let (text, base64) = if record.media_type.starts_with("audio/") {
            (
                None,
                Some(base64::engine::general_purpose::STANDARD.encode(&bytes)),
            )
        } else {
            match String::from_utf8(bytes) {
                Ok(text) => (Some(text), None),
                Err(error) => (
                    None,
                    Some(base64::engine::general_purpose::STANDARD.encode(error.as_bytes())),
                ),
            }
        };
        Ok(ContentPayload {
            record,
            text,
            base64,
        })
    }
}
#[cfg(test)]
fn read(db: &Connection, id: &str) -> Result<ContentPayload> {
    prepare(db, id)?.finish()
}

pub fn for_recording(db: &Connection, recording: &str) -> Result<Vec<ContentRecord>> {
    Ok(db
        .prepare(&format!(
            "SELECT {FIELDS} FROM ai_content_records WHERE recording_id=?1 ORDER BY rowid"
        ))?
        .query_map([recording], row)?
        .collect::<rusqlite::Result<_>>()?)
}
pub fn save_recording(db: &Connection, recording: &str, wav: &[u8]) -> Result<()> {
    if wav.is_empty() || wav.len() > 25 * 1024 * 1024 {
        return Err(invalid());
    }
    let id = format!("recording:{recording}:wav");
    let digest = super::results::digest(wav);
    let tx = db.unchecked_transaction()?;
    if !owner_exists(&tx, &Owner::Recording(recording.into()))? {
        return Err(invalid());
    }
    if let Some(existing) = for_recording(&tx, recording)?
        .into_iter()
        .find(|r| r.id == id)
    {
        return if existing.digest.as_ref() == Some(&digest) {
            Ok(())
        } else {
            Err(invalid())
        };
    }
    tx.execute("INSERT INTO ai_content_records(id,recording_id,kind,media_type,state,availability,byte_count,digest,last_used) VALUES(?1,?2,'recording.audio','audio/wav','complete','retained',?3,?4,?5)",params![id,recording,wav.len() as i64,digest,super::results::tick(&tx)?])?;
    tx.execute(
        "INSERT OR IGNORE INTO inference_blobs(digest,payload) VALUES(?1,?2)",
        params![digest, wav],
    )?;
    tx.execute(
        "INSERT INTO ai_content_chunks VALUES(?1,0,?2)",
        params![id, digest],
    )?;
    super::results::prune(&tx)?;
    tx.commit()?;
    Ok(())
}

mod base64_bytes {
    use base64::Engine;
    use serde::{Deserialize, Deserializer, Serializer};
    pub fn serialize<S: Serializer>(
        bytes: &[u8],
        serializer: S,
    ) -> std::result::Result<S::Ok, S::Error> {
        serializer.serialize_str(&base64::engine::general_purpose::STANDARD.encode(bytes))
    }
    pub fn deserialize<'de, D: Deserializer<'de>>(
        deserializer: D,
    ) -> std::result::Result<Vec<u8>, D::Error> {
        base64::engine::general_purpose::STANDARD
            .decode(String::deserialize(deserializer)?)
            .map_err(serde::de::Error::custom)
    }
}
