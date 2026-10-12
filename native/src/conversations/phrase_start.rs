//! Exact source text seeds a partner opening; it never creates learner evidence.
use crate::model::*;
use rusqlite::{Connection, OptionalExtension};

pub(crate) fn source(db: &Connection, message: &str, phrase: &str) -> Result<String> {
    if phrase.trim().is_empty()
        || phrase.chars().count() > 2000
        || phrase
            .chars()
            .any(|c| c.is_control() && !matches!(c, '\n' | '\r' | '\t'))
    {
        return Err(AppError::new(
            ErrorCode::Validation,
            "Select a phrase of 1–2,000 characters without control characters.",
        ));
    }
    let source: Option<(String, String)> = db.query_row(
        "SELECT m.conversation_id,m.text FROM messages m JOIN turns t ON t.id=m.turn_id
         JOIN conversations c ON c.id=m.conversation_id WHERE m.id=?1 AND m.role IN ('user','assistant')
         AND c.archived=0 AND t.state NOT IN ('cancelled','invalidated')
         AND EXISTS(SELECT 1 FROM turn_execution_owners o WHERE o.turn_id=t.id AND o.channel IN ('persona_reply','persona_opening'))
         AND NOT EXISTS(SELECT 1 FROM turns child WHERE child.replaces_turn_id=t.id)",
        [message], |r| Ok((r.get(0)?,r.get(1)?))).optional()?;
    let (conversation, text) = source.ok_or_else(|| {
        AppError::new(
            ErrorCode::Conflict,
            "The source message is no longer available.",
        )
    })?;
    if !text.contains(phrase) {
        return Err(AppError::new(
            ErrorCode::Conflict,
            "The selected phrase does not match the source message.",
        ));
    }
    Ok(conversation)
}

pub(crate) fn capture(
    db: &Connection,
    turn: &str,
    message: &str,
    phrase: &str,
    instructions: &str,
) -> Result<()> {
    capture_source(
        db,
        turn,
        serde_json::json!({"sourceMessageId":message}),
        phrase,
        instructions,
    )
}

pub(crate) fn capture_source(
    db: &Connection,
    turn: &str,
    source: serde_json::Value,
    phrase: &str,
    instructions: &str,
) -> Result<()> {
    let raw: String = db.query_row("SELECT context FROM turns WHERE id=?1", [turn], |r| {
        r.get(0)
    })?;
    let mut context: serde_json::Value = serde_json::from_str(&raw)?;
    let messages = context["messages"]
        .as_array_mut()
        .ok_or_else(|| AppError::new(ErrorCode::Storage, "Opening messages are missing."))?;
    let system = messages
        .first_mut()
        .ok_or_else(|| AppError::new(ErrorCode::Storage, "Opening instructions are missing."))?;
    let original = system["content"]
        .as_str()
        .ok_or_else(|| AppError::new(ErrorCode::Storage, "Opening instructions are invalid."))?;
    system["content"] = format!("{original}\n\n{instructions}").into();
    messages.push(serde_json::json!({"role":"user","content":serde_json::to_string(&serde_json::json!({"phrase":phrase}))?}));
    if messages
        .iter()
        .map(|m| m["content"].as_str().map_or(0, str::len))
        .sum::<usize>()
        > 96000
    {
        return Err(AppError::new(
            ErrorCode::Validation,
            "The phrase opening exceeds the conversation input budget.",
        ));
    }
    context["phraseSeed"] = source;
    context["phraseSeed"]["text"] = phrase.into();
    context["phraseSeed"]["policy"] = "exact-phrase-1".into();
    db.execute(
        "UPDATE turns SET context=?2 WHERE id=?1",
        rusqlite::params![turn, serde_json::to_string(&context)?],
    )?;
    Ok(())
}

pub(crate) fn validate(db: &Connection, turn: &str, text: &str) -> Result<()> {
    let phrase: Option<String> = db.query_row(
        "SELECT json_extract(context,'$.phraseSeed.text') FROM turns WHERE id=?1",
        [turn],
        |r| r.get(0),
    )?;
    if phrase.is_some_and(|phrase| !text.contains(&phrase)) {
        return Err(AppError::new(ErrorCode::Provider,"The opening did not contain the exact selected phrase. Retry the opening.")
            .with_diagnostics(serde_json::json!({"stage":"phrase_opening_validation","path":"text","reason":"exact_phrase_missing","expected":"opening containing the unchanged source phrase"})));
    }
    Ok(())
}
