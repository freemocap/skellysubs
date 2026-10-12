//! Private originals are additive. Historical absent payloads stay absent.
use super::*;
pub(super) fn apply(db: &Connection) -> Result<()> {
    v72_streaks::validate(db)?;
    db.execute_batch(include_str!("../../schemas/ai_content.sql"))?;
    Ok(())
}
pub(super) fn validate(db: &Connection) -> Result<()> {
    v72_streaks::validate(db)?;
    let reference = Connection::open_in_memory()?;
    reference.execute_batch(include_str!("../../schemas/ai_content.sql"))?;
    schema::validate_objects(db, &reference, false)
}
