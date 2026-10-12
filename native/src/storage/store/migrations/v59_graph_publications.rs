//! Add accepted-effect provenance without fabricating historical publications.
use super::*;

pub(super) fn apply(db: &Connection) -> Result<()> {
    v58_turn_execution_owners::validate(db)?;
    db.execute_batch(include_str!("v59_graph_publications.sql"))?;
    Ok(())
}

pub(super) fn validate(db: &Connection) -> Result<()> {
    v58_turn_execution_owners::validate(db)?;
    let reference = Connection::open_in_memory()?;
    reference.execute_batch(include_str!("v59_graph_publications.sql"))?;
    schema::validate_objects(db, &reference, false)
}
