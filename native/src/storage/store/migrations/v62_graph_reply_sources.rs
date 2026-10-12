//! Add optional reservations without rewriting historical messages or effects.
use super::*;
pub(super) fn apply(db: &Connection) -> Result<()> {
    v61_graph_reply_roles::validate(db)?;
    db.execute_batch(include_str!("v62_graph_reply_sources.sql"))?;
    Ok(())
}
pub(super) fn validate(db: &Connection) -> Result<()> {
    v61_graph_reply_roles::validate(db)?;
    let reference = Connection::open_in_memory()?;
    reference.execute_batch(include_str!("v61_graph_reply_roles.sql"))?;
    reference.execute_batch(include_str!("v62_graph_reply_sources.sql"))?;
    schema::validate_objects(db, &reference, false)
}
