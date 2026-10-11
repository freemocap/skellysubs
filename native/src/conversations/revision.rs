//! Transactional self-repair. Retain exact predecessor wording, remove only the
//! dependent later persona suffix; preserve private coach history, and never reuse a superseded operation's authority.
use crate::learning::coaching::InputEvidence;
use crate::model::*;
use rusqlite::{Connection, OptionalExtension, params};

pub(crate) fn suffix_counts(
    db: &Connection,
    conversation: &str,
    messages: &[ChatMessage],
) -> Result<Vec<RevisionSuffixCount>> {
    let targets: Vec<&str> = messages
        .iter()
        .filter(|m| m.role == "user" && m.replaced_by.is_none())
        .map(|m| m.turn_id.as_str())
        .collect();
    // One ordered scan computes suffix totals; only the bounded visible page is returned.
    Ok(db.prepare("WITH active AS (SELECT t.id,t.rowid AS ordering,EXISTS(SELECT 1 FROM turn_execution_owners WHERE turn_id=t.id AND channel='persona_reply') AS exchange FROM turns t WHERE conversation_id=?1 AND NOT EXISTS(SELECT 1 FROM turns child WHERE child.replaces_turn_id=t.id)), counts AS (SELECT id,exchange,coalesce(sum(exchange) OVER (ORDER BY ordering ROWS BETWEEN 1 FOLLOWING AND UNBOUNDED FOLLOWING),0) AS exchanges FROM active) SELECT id,exchanges,0 FROM counts WHERE exchange AND id IN (SELECT value FROM json_each(?2))")?.query_map(params![conversation,serde_json::to_string(&targets)?], |r| Ok(RevisionSuffixCount { turn_id:r.get(0)?, exchange_count:r.get(1)?, coach_turn_count:r.get(2)? }))?.collect::<rusqlite::Result<Vec<_>>>()?)
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn accept(
    db: &Connection,
    registry: &crate::configuration::Registry,
    snapshot: &Snapshot,
    conversation: &str,
    turn: &str,
    text: &str,
    mut input: InputEvidence,
    _expected: i32,
) -> Result<String> {
    let order: Option<i64> = db.query_row("SELECT rowid FROM turns t WHERE id=?1 AND conversation_id=?2 AND NOT EXISTS(SELECT 1 FROM turns child WHERE child.replaces_turn_id=t.id) AND EXISTS(SELECT 1 FROM turn_execution_owners WHERE turn_id=t.id AND channel='persona_reply')", params![turn,conversation], |r| r.get(0)).optional()?;
    let order = order.ok_or_else(|| {
        AppError::new(
            ErrorCode::Conflict,
            "Only the current version of a conversation exchange can be revised.",
        )
    })?;
    // Revising explicitly replaces this source and its dependent suffix. Workspace
    // activity is not a conflict; source identity above prevents competing edits.
    crate::speech::recording::transcription::validate_sources(
        db,
        conversation,
        input.recording_ids.as_deref().unwrap_or_default(),
    )?;
    if !matches!(input.modality.as_str(), "text" | "speech_transcript") {
        return Err(AppError::new(
            ErrorCode::Validation,
            "Invalid input modality.",
        ));
    }
    // Superseded native history may remain. Removing product ownership below
    // revokes publication; graph archives are not rewritten by message edits.
    db.execute("UPDATE turns SET state='invalidated' WHERE id=?1", [turn])?;
    // Capture one removal set for receipts, evidence choices and turns. Private
    // coach dialogue is history, not part of the regenerated persona exchange.
    let suffix: String = db.query_row(
        "SELECT coalesce(json_group_array(id),'[]') FROM turns WHERE conversation_id=?1 AND rowid>?2 AND NOT EXISTS(SELECT 1 FROM turn_execution_owners kept_coach WHERE kept_coach.turn_id=turns.id AND kept_coach.channel='coach')",
        params![conversation, order], |row| row.get(0),
    )?;
    // Action receipts contain submitted text too; suffix removal must remove
    // those source-owned copies, not merely their visible messages.
    db.execute("DELETE FROM receipts WHERE conversation_id=?1 AND (json_extract(receipt,'$.entityId') IN (SELECT id FROM turns WHERE conversation_id=?1 AND id IN (SELECT value FROM json_each(?2))) OR json_extract(request,'$.turnId') IN (SELECT id FROM turns WHERE conversation_id=?1 AND id IN (SELECT value FROM json_each(?2))))",params![conversation,suffix])?;
    // Exclusions are choices about retained observations. Remove only references
    // whose evidence is deleted with this suffix; keep predecessor exclusions.
    db.execute("UPDATE skill_choices SET excluded=(SELECT coalesce(json_group_array(value),'[]') FROM json_each(skill_choices.excluded) WHERE value NOT IN (SELECT coalesce(json_extract(context,'$.skillAssessmentAttempt'),id) FROM turns WHERE conversation_id=?1 AND id IN (SELECT value FROM json_each(?2)))),revision=revision+1 WHERE language_id=(SELECT language_id FROM conversations WHERE id=?1) AND EXISTS(SELECT 1 FROM json_each(skill_choices.excluded) WHERE value IN (SELECT coalesce(json_extract(context,'$.skillAssessmentAttempt'),id) FROM turns WHERE conversation_id=?1 AND id IN (SELECT value FROM json_each(?2))))",params![conversation,suffix])?;
    db.execute(
        "DELETE FROM turns WHERE conversation_id=?1 AND id IN (SELECT value FROM json_each(?2))",
        params![conversation, suffix],
    )?;
    let current = snapshot
        .conversations
        .iter()
        .find(|c| c.id == conversation)
        .ok_or_else(|| AppError::new(ErrorCode::NotFound, "Conversation no longer exists."))?;
    let replacement = crate::conversations::execution::accept_revision_send(
        db,
        registry,
        snapshot,
        conversation,
        text,
        current.revision,
        turn,
    )?;
    input.revision = true;
    db.execute("UPDATE turns SET replaces_turn_id=?2,context=json_set(context,'$.input',json(?3)) WHERE id=?1", params![replacement,turn,serde_json::to_string(&input)?])?;
    crate::learning::effort::revision(db, &replacement, turn)?;
    Ok(replacement)
}

/// Project the retained revision chain into coach context; no duplicate event store.
pub(crate) fn coach_edits(db: &Connection, conversation: &str) -> Result<Vec<serde_json::Value>> {
    let rows = db.prepare("SELECT t.id,t.replaces_turn_id,old.text,new.text FROM turns t JOIN messages old ON old.turn_id=t.replaces_turn_id AND old.role='user' JOIN messages new ON new.turn_id=t.id AND new.role='user' WHERE t.conversation_id=?1 ORDER BY t.rowid DESC LIMIT 10")?
        .query_map([conversation], |r| Ok(serde_json::json!({"type":"message_edited", "turnId":r.get::<_,String>(0)?, "replacesTurnId":r.get::<_,String>(1)?, "before":r.get::<_,String>(2)?, "after":r.get::<_,String>(3)?})))?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    Ok(rows)
}
