//! Domain ownership and channel identity, independent of graph scheduling state.
use crate::model::{AppError, ErrorCode, Result};
use rusqlite::Connection;

#[cfg(test)]
mod tests;

pub(crate) fn validate_complete(db: &Connection) -> Result<()> {
    let missing: bool = db.query_row("SELECT EXISTS(SELECT 1 FROM turns t LEFT JOIN turn_execution_owners o ON o.turn_id=t.id WHERE o.turn_id IS NULL)", [], |r| r.get(0))?;
    if missing {
        return Err(AppError::new(
            ErrorCode::Storage,
            "A retained turn has no execution owner.",
        ));
    }
    let mismatched: bool = db.query_row("SELECT EXISTS(SELECT 1 FROM turn_execution_owners o JOIN turns t ON t.id=o.turn_id JOIN graph_engines e ON e.id=o.engine_id WHERE t.conversation_id!=e.conversation_id)", [], |r| r.get(0))?;
    if mismatched {
        return Err(AppError::new(
            ErrorCode::Storage,
            "A retained graph and turn have different owners.",
        ));
    }
    Ok(())
}
