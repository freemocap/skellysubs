//! Native speech receipts and evictable payload retention. Callers own native
//! settlement/adoption authority and the enclosing transaction; no provider calls
//! or graph state are inferred from cache presence.
use super::{
    alignment::{SpeechAlignment, SpeechAudio},
    synthesis_graph::{Receipt, Request},
};
use crate::{
    ai::{graph::InvocationIdentity, results},
    model::{AppError, ErrorCode, Result},
};
use rusqlite::{Connection, OptionalExtension, params};

fn invalid(message: &str) -> AppError {
    AppError::new(ErrorCode::Storage, message)
}
fn transaction(db: &Connection) -> Result<()> {
    if db.is_autocommit() {
        return Err(invalid("Graph audio requires the native transaction."));
    }
    Ok(())
}
pub fn reference(identity: &InvocationIdentity, wav: &[u8]) -> Result<Receipt> {
    let id = producer_id(identity)?;
    if identity.operation != super::synthesis_graph::operation_contract() {
        return Err(invalid("Graph audio requires a synthesis producer."));
    }
    let engine = identity
        .engine
        .clone()
        .filter(|s| !s.is_empty())
        .ok_or_else(|| invalid("Graph audio requires a durable producer."))?;
    if wav.is_empty() || wav.len() > super::delivery::AUDIO_LIMIT {
        return Err(invalid("Graph audio exceeds delivery limits."));
    }
    let mut reader = hound::WavReader::new(std::io::Cursor::new(wav))
        .map_err(|_| invalid("Graph audio is not a valid WAV payload."))?;
    let spec = reader.spec();
    if spec.channels != 1
        || spec.bits_per_sample != 16
        || spec.sample_format != hound::SampleFormat::Int
        || spec.sample_rate == 0
        || reader.duration() == 0
        || reader.samples::<i16>().any(|sample| sample.is_err())
    {
        return Err(invalid("Graph audio requires complete PCM16 mono samples."));
    }
    let execution = serde_json::to_value(identity.execution)?.to_string();
    Ok(Receipt {
        id,
        engine,
        execution,
        digest: results::digest(wav),
        bytes: wav.len() as u64,
    })
}
pub fn producer_id(identity: &InvocationIdentity) -> Result<String> {
    if identity.operation != super::synthesis_graph::operation_contract() {
        return Err(invalid("Graph audio requires a synthesis producer."));
    }
    let engine = identity
        .engine
        .as_deref()
        .filter(|s| !s.is_empty())
        .ok_or_else(|| invalid("Graph audio requires a durable producer."))?;
    Ok(format!(
        "graph-audio:{}",
        serde_json::to_string(&(engine, serde_json::to_string(&identity.execution)?))?
    ))
}
/// Invoke in the same transaction as the native settlement containing this
/// reference. Metadata/billing observations belong to the native execution log.
pub fn retain(
    db: &Connection,
    identity: &InvocationIdentity,
    request: &Request,
    wav: &[u8],
    alignment: Option<&SpeechAlignment>,
) -> Result<Receipt> {
    transaction(db)?;
    let receipt = reference(identity, wav)?;
    if alignment.is_some_and(|a| !a.valid() || a.source_text != request.source.text) {
        return Err(invalid(
            "Graph audio alignment does not match its exact source.",
        ));
    }
    let raw_alignment = alignment.map(serde_json::to_string).transpose()?;
    let conversation_engine: bool = db.query_row(
        "SELECT EXISTS(SELECT 1 FROM graph_engines WHERE id=?1)",
        [&receipt.engine],
        |r| r.get(0),
    )?;
    let prefix = if conversation_engine {
        ""
    } else {
        "workspace_"
    };
    db.execute(&format!("INSERT INTO {prefix}graph_audio_receipts(id,engine_id,execution_id,audio_digest,audio_bytes,alignment) VALUES(?1,?2,?3,?4,?5,?6)"), params![receipt.id,receipt.engine,receipt.execution,receipt.digest,receipt.bytes as i64,raw_alignment])?;
    let payload = serde_json::to_vec(&SpeechAudio::new(wav, alignment.cloned()))?;
    if payload.len() as u64 <= results::settings(db)?.capacity_bytes {
        let digest = results::digest(&payload);
        let scope = results::speech::scope(&request.settings.target, &request.settings.install_id)?;
        let key = results::speech::request_key(&scope, &request.speech_input())?;
        db.execute(
            "INSERT OR IGNORE INTO inference_blobs(digest,payload) VALUES(?1,?2)",
            params![digest, payload],
        )?;
        db.execute(&format!("INSERT INTO {prefix}graph_audio_cache(receipt_id,request_key,blob_digest,last_used) VALUES(?1,?2,?3,?4)"), params![receipt.id,key,digest,results::tick(db)?])?;
        super::analysis::signal_cache::retain(db, "inference", &receipt.id, wav)?;
    }
    results::prune(db)?;
    Ok(receipt)
}

/// A receipt is independently inspectable even after eviction. Adoption must
/// verify it against the exact output and producer, not just an opaque ID.
pub fn verify(db: &Connection, expected: &Receipt) -> Result<()> {
    transaction(db)?;
    let found: bool = db.query_row("SELECT EXISTS(SELECT 1 FROM native_graph_audio_receipts WHERE id=?1 AND engine_id=?2 AND execution_id=?3 AND audio_digest=?4 AND audio_bytes=?5)", params![expected.id,expected.engine,expected.execution,expected.digest,i64::try_from(expected.bytes).map_err(|_| invalid("Invalid audio byte count."))?], |r| r.get(0))?;
    if !found {
        return Err(invalid("Native audio receipt is missing or mismatched."));
    }
    Ok(())
}

pub struct CachedAudio {
    pub receipt: Receipt,
    pub wav: Vec<u8>,
    pub alignment: Option<SpeechAlignment>,
}
/// Lookup is requested explicitly. A cache miss has no synthesis side effect.
pub fn lookup(db: &Connection, request: &Request) -> Result<Option<CachedAudio>> {
    transaction(db)?;
    let scope = results::speech::scope(&request.settings.target, &request.settings.install_id)?;
    let key = results::speech::request_key(&scope, &request.speech_input())?;
    let id: Option<String> = db.query_row("SELECT receipt_id FROM native_graph_audio_cache WHERE request_key=?1 ORDER BY last_used DESC,receipt_id LIMIT 1", [key], |r| r.get(0)).optional()?;
    id.map(|id| read(db, &id)).transpose().map(Option::flatten)
}
pub fn read(db: &Connection, id: &str) -> Result<Option<CachedAudio>> {
    transaction(db)?;
    type SavedAudio = (
        String,
        String,
        String,
        u32,
        Option<String>,
        String,
        Vec<u8>,
        bool,
    );
    let saved: Option<SavedAudio> = db.query_row("SELECT r.engine_id,r.execution_id,r.audio_digest,r.audio_bytes,r.alignment,c.blob_digest,b.payload,r.workspace FROM native_graph_audio_receipts r JOIN native_graph_audio_cache c ON c.receipt_id=r.id JOIN inference_blobs b ON b.digest=c.blob_digest WHERE r.id=?1", [id], |r| Ok((r.get(0)?,r.get(1)?,r.get(2)?,r.get(3)?,r.get(4)?,r.get(5)?,r.get(6)?,r.get(7)?))).optional()?;
    let Some((engine, execution, digest, bytes, alignment, blob_digest, payload, workspace)) =
        saved
    else {
        return Ok(None);
    };
    if results::digest(&payload) != blob_digest {
        return Err(invalid("Graph audio cache integrity failed."));
    }
    let audio = SpeechAudio::decode(&payload)?;
    let wav = audio.wav()?;
    let alignment: Option<SpeechAlignment> =
        alignment.map(|s| serde_json::from_str(&s)).transpose()?;
    if results::digest(&wav) != digest
        || wav.len() as u32 != bytes
        || audio.alignment != alignment
        || alignment.as_ref().is_some_and(|a| !a.valid())
    {
        return Err(invalid("Graph audio does not match its durable receipt."));
    }
    db.execute(
        if workspace {
            "UPDATE workspace_graph_audio_cache SET last_used=?2 WHERE receipt_id=?1"
        } else {
            "UPDATE graph_audio_cache SET last_used=?2 WHERE receipt_id=?1"
        },
        params![id, results::tick(db)?],
    )?;
    super::analysis::signal_cache::retain(db, "inference", id, &wav)?;
    Ok(Some(CachedAudio {
        receipt: Receipt {
            id: id.into(),
            engine,
            execution,
            digest,
            bytes: bytes.into(),
        },
        wav,
        alignment,
    }))
}

#[cfg(test)]
mod tests;
