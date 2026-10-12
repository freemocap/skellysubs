//! Product turn status is a projection of native facts. Optional failures do not
//! prevent scheduling independent nodes or erase their individual failure states.
use super::*;
use rusqlite::OptionalExtension;

pub(super) fn partner(db: &Connection, engine: &DurableEngine, run: &str) -> Result<bool> {
    let owner:Option<(String,String,String)> = db.query_row("SELECT t.id,t.state,e.node_key FROM turn_execution_owners o JOIN turns t ON t.id=o.turn_id JOIN conversation_graph_effects e ON e.turn_id=t.id WHERE o.engine_id=?1 AND o.run_id=?2 AND o.channel IN ('persona_reply','persona_opening')",params![engine.stamp().engine,run],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?))).optional()?;
    let Some((turn, previous, reply)) = owner else {
        return Ok(false);
    };
    if matches!(previous.as_str(), "cancelled" | "invalidated") {
        return Ok(true);
    }
    let view = engine.inspect(run).map_err(error)?;
    let primary = view.nodes.get(&reply).ok_or_else(|| {
        AppError::new(
            ErrorCode::Storage,
            "Native reply effect has no executable node.",
        )
    })?;
    let state = match primary {
        Disposition::Adopted => {
            if view.nodes.values().any(|state| {
                matches!(
                    state,
                    Disposition::Ready
                        | Disposition::Paused
                        | Disposition::Held
                        | Disposition::Prepared
                        | Disposition::Running
                        | Disposition::Available
                )
            }) {
                "assisting"
            } else {
                "succeeded"
            }
        }
        Disposition::Unknown => "unknown",
        Disposition::Failed | Disposition::Blocked => "failed",
        _ => "pending",
    };
    if state != previous {
        db.execute(
            "UPDATE turns SET state=?2 WHERE id=?1",
            params![turn, state],
        )?;
        super::super::bump(db)?;
    }
    Ok(true)
}
