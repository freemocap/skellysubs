//! Exploration is new, explicitly requested information, never a view or skill score.
use super::{EffortDimension, award};
use crate::{
    language::reading::{ReadingAid, Request},
    model::Result,
};
use rusqlite::{Connection, params};
use serde_json::{Value, json};

/// Award an adopted coach inquiry through its immutable domain effect.
/// The recorded source and policy determine attribution and deduplication.
pub(crate) fn graph_coach_reply(db: &Connection, effect: &str) -> Result<()> {
    let (source, language, variety, conversation): (String, String, String, String) = db.query_row(
        "SELECT e.award_source,e.language_id,e.variety_id,t.conversation_id FROM conversation_graph_effects e JOIN conversation_graph_publications p ON p.effect_id=e.id JOIN turn_execution_owners o ON o.turn_id=e.turn_id JOIN turns t ON t.id=e.turn_id WHERE e.id=?1 AND e.role='coach_reply' AND o.executor='graph' AND o.channel='coach'",
        [effect], |r| Ok((r.get(0)?,r.get(1)?,r.get(2)?,r.get(3)?)),
    )?;
    award(
        db,
        EffortDimension::Explorations,
        &source,
        &language,
        &variety,
        Some(&conversation),
    )
}

/// Only accepted fresh text generation qualifies. A stable content identity prevents
/// repairs, explicit retries and cache eviction from minting another point for the same aid.
pub(crate) fn reading(
    db: &Connection,
    request: &Request,
    receipt: &Value,
) -> Result<Option<super::EffortAward>> {
    if request.input.aid == ReadingAid::Speech
        || receipt["state"] != "succeeded"
        || receipt["response"]["cacheHit"] != false
    {
        return Ok(None);
    }
    let source = format!(
        "reading:{}",
        crate::ai::results::digest(&serde_json::to_vec(&json!([
            request.input.text,
            request.input.aid,
            request.context.language_id,
            request.context.variety_id,
            request.context.explanation_language_id,
            request.context.explanation_variety_id
        ]))?)
    );
    // Attribution was captured before dispatch; a later language selection cannot move it.
    award(
        db,
        EffortDimension::Explorations,
        &source,
        &request.context.language_id,
        &request.context.variety_id,
        request.input.conversation_id.as_deref(),
    )?;
    if db.changes() == 0 {
        return Ok(None);
    }
    Ok(super::read_scoped(db, &request.context.language_id, None)?
        .recent
        .into_iter()
        .find(|item| item.source_id == source))
}

pub(crate) fn validate_conversation(
    db: &Connection,
    conversation: Option<&str>,
    language: &str,
) -> Result<()> {
    if let Some(id) = conversation {
        let valid: bool = db.query_row("SELECT EXISTS(SELECT 1 FROM conversations WHERE id=?1 AND language_id=?2 AND archived=0)", params![id, language], |r| r.get(0))?;
        if !valid {
            return Err(crate::model::AppError::new(
                crate::model::ErrorCode::Validation,
                "Exploration conversation is unavailable or belongs to another language.",
            ));
        }
    }
    Ok(())
}

/// One accepted set of newly generated practice material, independent of adoption.
pub(crate) fn practice_proposal(
    db: &Connection,
    request: &crate::ai::generation::Request,
) -> Result<()> {
    let context = &request.language_context;
    award(
        db,
        EffortDimension::Explorations,
        &format!("practice:{}", request.id),
        &context.language_id,
        &context.variety_id,
        None,
    )
}

/// Repeated attempts for the same source/note cannot create another exploration.
pub(crate) fn graph_feedback(db: &Connection, turn: &str, note: &str) -> Result<()> {
    let (conversation,language,variety):(String,String,String)=db.query_row("SELECT t.conversation_id,c.language_id,json_extract(t.context,'$.practiceSettings.varietyId') FROM turns t JOIN conversations c ON c.id=t.conversation_id WHERE t.id=?1",[turn],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?)))?;
    let source = format!(
        "graph-feedback:{turn}:{}",
        crate::ai::results::digest(&serde_json::to_vec(note)?)
    );
    award(
        db,
        EffortDimension::Explorations,
        &source,
        &language,
        &variety,
        Some(&conversation),
    )
}
