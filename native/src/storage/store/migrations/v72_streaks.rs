//! Prospective streak tracking adds no inferred historical activity or XP.
use super::*;

pub(super) fn apply(db: &Connection) -> Result<()> {
    v71_cache::validate(db)?;
    db.execute_batch(include_str!("v72_streaks.sql"))?;
    Ok(())
}

pub(super) fn validate(db: &Connection) -> Result<()> {
    v71_cache::validate(db)?;
    let reference = Connection::open_in_memory()?;
    reference.execute_batch(include_str!("v72_streaks.sql"))?;
    schema::validate_objects(db, &reference, false)
}
