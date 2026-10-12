use super::*;

pub(super) fn check(db: &Connection, action: &Action) -> Result<()> {
    let native = match action {
        Action::RequestMessageHelp { message_id, .. }
        | Action::RequestMessageSpeech { message_id, .. }
        | Action::RequestExplanations { message_id }
        | Action::RequestSuggestions { message_id }
        | Action::RetryReplyHelp { message_id, .. } => db.query_row(
            "SELECT EXISTS(SELECT 1 FROM messages m JOIN turn_execution_owners o ON o.turn_id=m.turn_id WHERE m.id=?1 AND o.executor='graph')",
            [message_id], |r| r.get(0),
        )?,
        Action::ReassessFeedback { turn_id, .. }
        | Action::ControlTurn { turn_id, control: TurnControl::Resume | TurnControl::Step | TurnControl::Retry } => db.query_row(
            "SELECT EXISTS(SELECT 1 FROM turn_execution_owners WHERE turn_id=?1 AND executor='graph')",
            [turn_id], |r| r.get(0),
        )?,
        Action::RetryGloss { operation_id } => operation_id.starts_with("graph:"),
        // Disclosure, cancellation, saved reads and new native turns remain usable.
        _ => true,
    };
    if native {
        Ok(())
    } else {
        Err(AppError::new(
            ErrorCode::Conflict,
            "Execution owner is unavailable.",
        ))
    }
}
