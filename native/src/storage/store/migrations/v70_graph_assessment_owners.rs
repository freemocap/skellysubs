//! Preserve result/disclosure identities while making producer ownership explicit.
use super::*;
pub(super) fn apply(db: &Connection) -> Result<()> {
    v69_graph_helper_requests::validate(db)?;
    db.execute_batch("CREATE TEMP TABLE saved_assessments_70 AS SELECT a.*,o.engine_id,o.run_id FROM conversation_graph_assessments a LEFT JOIN turn_execution_owners o ON o.turn_id=a.turn_id; CREATE TEMP TABLE saved_disclosures_70 AS SELECT * FROM conversation_graph_disclosures; DROP TABLE conversation_graph_disclosures; DROP TABLE conversation_graph_assessments;")?;
    db.execute_batch(include_str!("v70_graph_assessment_owners.sql"))?;
    db.execute_batch("INSERT INTO conversation_graph_assessments SELECT * FROM saved_assessments_70; INSERT INTO conversation_graph_disclosures SELECT * FROM saved_disclosures_70; DROP TABLE saved_disclosures_70; DROP TABLE saved_assessments_70;")?;
    Ok(())
}
pub(super) fn validate(db: &Connection) -> Result<()> {
    v62_graph_reply_sources::validate(db)?;
    let reference = Connection::open_in_memory()?;
    for sql in [
        include_str!("v57_graph_runtime.sql"),
        include_str!("v64_graph_audio.sql"),
        include_str!("v65_graph_audio_delivery.sql"),
        include_str!("v67_graph_speech_requests.sql"),
        include_str!("v68_workspace_graphs.sql"),
        include_str!("v69_graph_helper_requests.sql"),
        include_str!("v70_graph_assessment_owners.sql"),
    ] {
        reference.execute_batch(sql)?;
    }
    schema::validate_objects(db, &reference, false)
}
