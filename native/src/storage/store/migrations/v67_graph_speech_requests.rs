//! Add message-owned playback requests without changing existing records.
use super::*;
pub(super) fn apply(db: &Connection) -> Result<()> {
    v65_graph_audio_delivery::validate(db)?;
    db.execute_batch(include_str!("v67_graph_speech_requests.sql"))?;
    Ok(())
}
pub(super) fn validate(db: &Connection) -> Result<()> {
    v65_graph_audio_delivery::validate(db)?;
    let reference = Connection::open_in_memory()?;
    reference.execute_batch(include_str!("v57_graph_runtime.sql"))?;
    reference.execute_batch(include_str!("v67_graph_speech_requests.sql"))?;
    schema::validate_objects(db, &reference, false)
}
