//! Native assessment receipts are additive; existing assessments/disclosures stay intact.
use super::*;
pub(super) fn apply(db: &Connection) -> Result<()> {
    v62_graph_reply_sources::validate(db)?;
    db.execute_batch(include_str!("v63_graph_assessments.sql"))?;
    Ok(())
}
pub(super) fn validate(db: &Connection) -> Result<()> {
    v62_graph_reply_sources::validate(db)?;
    let reference = Connection::open_in_memory()?;
    reference.execute_batch(include_str!("v63_graph_assessments.sql"))?;
    if schema::validate_objects(db, &reference, false).is_ok() {
        return Ok(());
    }
    // Format 63 existed before numeric ID constraints were tightened in place.
    // Accept that exact historical DDL as well; format 70 rebuilds both variants
    // under the current constraints inside the normal migration transaction.
    let original = Connection::open_in_memory()?;
    original.execute_batch(include_str!("v63_graph_assessments_original.sql"))?;
    schema::validate_objects(db, &original, false)
}
