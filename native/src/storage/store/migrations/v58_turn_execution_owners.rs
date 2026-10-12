//! Retain executor/channel evidence without inventing graphs for historical turns.
use super::*;

pub(super) fn apply(db: &Connection) -> Result<()> {
    v57_graph_runtime::validate(db)?;
    let ambiguous: bool = db.query_row("SELECT EXISTS(SELECT 1 FROM operations WHERE kind IN ('coach_reply','persona_reply','persona_opening') GROUP BY turn_id HAVING count(*)>1)", [], |r| r.get(0))?;
    if ambiguous {
        return Err(AppError::new(
            ErrorCode::Storage,
            "Conflicting historical turn channels; no execution owner was inferred.",
        ));
    }
    db.execute_batch(include_str!("v58_turn_execution_owners.sql"))?;
    db.execute_batch("INSERT INTO turn_execution_owners(turn_id,executor,channel)
        SELECT t.id,'legacy',COALESCE((SELECT CASE o.kind WHEN 'coach_reply' THEN 'coach' ELSE o.kind END FROM operations o WHERE o.turn_id=t.id AND o.kind IN ('coach_reply','persona_reply','persona_opening')),'unknown') FROM turns t;")?;
    Ok(())
}

pub(super) fn validate(db: &Connection) -> Result<()> {
    v57_graph_runtime::validate(db)?;
    let reference = Connection::open_in_memory()?;
    reference.execute_batch(include_str!("v45.sql"))?;
    reference.execute_batch(include_str!("v57_graph_runtime.sql"))?;
    reference.execute_batch(include_str!("v58_turn_execution_owners.sql"))?;
    schema::validate_objects(db, &reference, false)?;
    crate::conversations::execution_owner::validate_complete(db)
}
