//! Add producer wire identities without inventing requests for historical runs.
use super::*;

pub(super) fn apply(db: &Connection) -> Result<()> {
    v59_graph_publications::validate(db)?;
    db.execute_batch(include_str!("v60_graph_transport.sql"))?;
    Ok(())
}

pub(super) fn validate(db: &Connection) -> Result<()> {
    v59_graph_publications::validate(db)?;
    let reference = Connection::open_in_memory()?;
    reference.execute_batch(include_str!("v60_graph_transport.sql"))?;
    schema::validate_objects(db, &reference, false)
}
