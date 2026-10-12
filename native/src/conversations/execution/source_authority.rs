//! Shared current source-ownership policy for captured conversation prompts.
use super::*;

pub(super) fn check(db: &Connection, turn: &str, sources: &[String]) -> Result<()> {
    for source in sources {
        let permitted: bool = db.query_row(
            "SELECT EXISTS(SELECT 1 FROM messages m JOIN turns t ON t.conversation_id=m.conversation_id WHERE m.id=?1 AND t.id=?2)",
            params![source, turn], |row| row.get(0))?;
        if !permitted {
            return Err(fail(
                "A captured conversation source is unavailable or outside this turn's scope.",
            ));
        }
    }
    Ok(())
}
