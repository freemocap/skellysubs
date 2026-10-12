//! Workspace-owned native engines for workflows without a conversation.
use super::*;
pub(super) fn apply(db: &Connection) -> Result<()> {
    v67_graph_speech_requests::validate(db)?;
    db.execute_batch(include_str!("v68_workspace_graphs.sql"))?;
    Ok(())
}
pub(super) fn validate(db: &Connection) -> Result<()> {
    v67_graph_speech_requests::validate(db)?;
    let reference = Connection::open_in_memory()?;
    reference.execute_batch(include_str!("v68_workspace_graphs.sql"))?;
    schema::validate_objects(db, &reference, false)
}
