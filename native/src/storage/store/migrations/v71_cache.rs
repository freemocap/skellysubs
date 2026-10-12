//! Cache payload ownership belongs to graph runs and audio receipts.
use super::*;
pub(super) fn apply(db: &Connection) -> Result<()> {
    v70_graph_assessment_owners::validate(db)?;
    db.execute_batch("DROP TABLE inference_consumers; DROP TABLE inference_results; DROP TABLE inference_executions;")?;
    Ok(())
}
pub(super) fn validate(db: &Connection) -> Result<()> {
    let reference = Connection::open_in_memory()?;
    reference.execute_batch(include_str!("v71_cache.sql"))?;
    schema::validate_objects(db, &reference, false)?;
    let remaining: i64 = db.query_row("SELECT count(*) FROM sqlite_master WHERE name IN ('inference_consumers','inference_results','inference_executions','inference_result_request','inference_result_recency')", [], |r|r.get(0))?;
    if remaining != 0 {
        return Err(AppError::new(
            ErrorCode::Storage,
            "Unexpected execution-cache objects.",
        ));
    }
    Ok(())
}
