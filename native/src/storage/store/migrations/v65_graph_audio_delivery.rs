//! Add consumer-owned delivery evidence; retain all prior execution/history rows.
use super::*;
pub(super) fn apply(db: &Connection) -> Result<()> {
    v64_graph_audio::validate(db)?;
    db.execute_batch(include_str!("v65_graph_audio_delivery.sql"))?;
    Ok(())
}
pub(super) fn validate(db: &Connection) -> Result<()> {
    v64_graph_audio::validate(db)?;
    let reference = Connection::open_in_memory()?;
    reference.execute_batch(include_str!("v65_graph_audio_delivery.sql"))?;
    schema::validate_objects(db, &reference, false)
}
