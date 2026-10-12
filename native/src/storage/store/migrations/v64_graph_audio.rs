//! Add native speech provenance without rewriting legacy receipts or cache entries.
use super::*;
pub(super) fn apply(db: &Connection) -> Result<()> {
    v63_graph_assessments::validate(db)?;
    db.execute_batch(include_str!("v64_graph_audio.sql"))?;
    Ok(())
}
pub(super) fn validate(db: &Connection) -> Result<()> {
    v63_graph_assessments::validate(db)?;
    let reference = Connection::open_in_memory()?;
    reference.execute_batch(include_str!("v64_graph_audio.sql"))?;
    schema::validate_objects(db, &reference, false)
}
