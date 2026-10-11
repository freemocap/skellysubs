//! Add lookup indexes without changing retained content, identity or ownership.
use super::*;

pub(super) fn apply(db: &Connection) -> Result<()> {
    v74_content::validate(db)?;
    db.execute_batch(include_str!("../../schemas/inference_blob_references.sql"))?;
    Ok(())
}

pub(super) fn validate(db: &Connection) -> Result<()> {
    v74_content::validate(db)?;
    let reference = Connection::open_in_memory()?;
    reference.execute_batch(include_str!("v71_cache.sql"))?;
    reference.execute_batch(include_str!("../../schemas/graph_audio.sql"))?;
    reference.execute_batch(include_str!("../../schemas/workspace_graph_audio.sql"))?;
    reference.execute_batch(include_str!("../../schemas/workspace_reading_cache.sql"))?;
    reference.execute_batch(include_str!(
        "../../schemas/workspace_transcription_cache.sql"
    ))?;
    reference.execute_batch(include_str!("../../schemas/ai_content.sql"))?;
    reference.execute_batch(include_str!("../../schemas/inference_blob_references.sql"))?;
    let mut indexes = reference.prepare("SELECT name,sql FROM sqlite_master WHERE type='index' AND name IN ('ai_content_chunk_blob','graph_audio_cache_blob','workspace_graph_audio_cache_blob','workspace_reading_cache_blob','workspace_transcription_cache_blob')")?;
    for row in indexes.query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)))? {
        let (name, expected) = row?;
        let actual: Option<String> = db
            .query_row(
                "SELECT sql FROM sqlite_master WHERE type='index' AND name=?1",
                [&name],
                |r| r.get(0),
            )
            .optional()?;
        if actual.as_deref() != Some(expected.as_str()) {
            return Err(AppError::new(
                ErrorCode::Storage,
                format!("Unexpected cache reference index: {name}."),
            ));
        }
    }
    Ok(())
}
