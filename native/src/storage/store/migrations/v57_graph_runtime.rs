//! Add empty native graph storage without interpreting historical workflows.
use super::*;

pub(super) fn apply(db: &Connection) -> Result<()> {
    v56_reading_review::validate(db)?;
    db.execute_batch(include_str!("v57_graph_runtime.sql"))?;
    Ok(())
}

pub(super) fn validate(db: &Connection) -> Result<()> {
    v56_reading_review::validate(db)?;
    let reference = Connection::open_in_memory()?;
    reference.execute_batch(include_str!("v57_graph_runtime.sql"))?;
    schema::validate_objects(db, &reference, false)
}
