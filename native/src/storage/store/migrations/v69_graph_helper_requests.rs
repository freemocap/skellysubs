//! Add message-owned fresh helpers; previous results and owners remain intact.
use super::*;
pub(super) fn apply(db: &Connection) -> Result<()> {
    v68_workspace_graphs::validate(db)?;
    db.execute_batch(include_str!("v69_graph_helper_requests.sql"))?;
    Ok(())
}
pub(super) fn validate(db: &Connection) -> Result<()> {
    v65_graph_audio_delivery::validate(db)?;
    let reference = Connection::open_in_memory()?;
    for sql in [
        include_str!("v57_graph_runtime.sql"),
        include_str!("v67_graph_speech_requests.sql"),
        include_str!("v68_workspace_graphs.sql"),
        include_str!("v69_graph_helper_requests.sql"),
    ] {
        reference.execute_batch(sql)?;
    }
    schema::validate_objects(db, &reference, false)
}
