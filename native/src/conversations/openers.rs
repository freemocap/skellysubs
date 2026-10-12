//! Starts capture selected direction atomically, without manufacturing learner evidence.
use crate::configuration::Registry;
use crate::{conversations::direction::*, model::*};
use rusqlite::{Connection, OptionalExtension, params};
pub(crate) fn selected(db: &Connection, conversation: &str) -> Result<Option<Opening>> {
    let raw: Option<String> = db
        .query_row(
            "SELECT opening FROM conversation_openings WHERE conversation_id=?1",
            [conversation],
            |r| r.get(0),
        )
        .optional()?;
    raw.map(|s| serde_json::from_str(&s).map_err(Into::into))
        .transpose()
}
/// The starter cards for one conversation: each scene named in the target
/// language, transliterated where that variety resolves to a romanization
/// scheme, and named again in the explanation language.
///
/// Content completeness is enforced at registry load, so a missing label here is
/// a bug in validation rather than a learner-visible blank.
pub(crate) fn choices(
    registry: &Registry,
    target: &str,
    variety: Option<&str>,
    explanation: &str,
) -> Result<Vec<TopicCard>> {
    let scheme = registry.active_romanization_scheme(target, variety)?;
    let label = |id: &str, labels: &std::collections::BTreeMap<String, String>, language: &str| {
        labels.get(language).cloned().ok_or_else(|| {
            AppError::new(
                ErrorCode::Validation,
                format!("Topic {id} has no label for {language}."),
            )
        })
    };
    registry
        .topics()
        .iter()
        .map(|t| {
            Ok(TopicCard {
                id: t.id.clone(),
                glyph: t.glyph.clone(),
                target: label(&t.id, &t.labels, target)?,
                romanized: match &scheme {
                    Some(key) => Some(t.romanizations.get(key).cloned().ok_or_else(|| {
                        AppError::new(
                            ErrorCode::Validation,
                            format!("Topic {} has no {key} romanization.", t.id),
                        )
                    })?),
                    None => None,
                },
                translation: label(&t.id, &t.labels, explanation)?,
            })
        })
        .collect()
}
pub(crate) fn accept_native(
    db: &Connection,
    snapshot: &Snapshot,
    registry: &Registry,
    conversation: &str,
    configuration: ConversationStartConfig,
    learner_message: Option<(String, crate::learning::coaching::InputEvidence)>,
    expected: i32,
) -> Result<String> {
    accept_captured(
        db,
        snapshot,
        registry,
        conversation,
        configuration,
        learner_message,
        expected,
    )
}
#[allow(clippy::too_many_arguments)]
fn accept_captured(
    db: &Connection,
    snapshot: &Snapshot,
    registry: &Registry,
    conversation: &str,
    configuration: ConversationStartConfig,
    learner_message: Option<(String, crate::learning::coaching::InputEvidence)>,
    expected: i32,
) -> Result<String> {
    if snapshot.revision != expected {
        return Err(AppError::new(
            ErrorCode::Conflict,
            "The conversation changed. Review it before starting.",
        ));
    }
    let occupied:bool=db.query_row("SELECT EXISTS(SELECT 1 FROM turns WHERE conversation_id=?1 AND (state='pending' OR EXISTS(SELECT 1 FROM turn_execution_owners WHERE turn_id=turns.id AND channel IN ('persona_reply','persona_opening'))))",[conversation],|r|r.get(0))?;
    if occupied || selected(db, conversation)?.is_some() {
        return Err(AppError::new(
            ErrorCode::Conflict,
            "This conversation has already started.",
        ));
    }
    let mut captured = snapshot.clone();
    let current = captured
        .conversations
        .iter_mut()
        .find(|c| c.id == conversation)
        .ok_or_else(|| AppError::new(ErrorCode::NotFound, "Conversation not found."))?;
    current.settings = settings(
        registry,
        &current.language_id,
        &current.settings,
        &configuration,
    )?;
    db.execute(
        "UPDATE conversation_settings SET settings=?2,revision=revision+1 WHERE conversation_id=?1",
        params![conversation, serde_json::to_string(&current.settings)?],
    )?;
    current.settings_revision += 1;
    let revision = current.revision;
    let opening = if learner_message.is_some() {
        Opening::Learner
    } else {
        Opening::Partner
    };
    let turn = if let Some((text, input)) = learner_message {
        let turn = crate::conversations::execution::capture_native_send(
            db,
            registry,
            &captured,
            conversation,
            &text,
            revision,
        )?;
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
        db.execute(
            "UPDATE turns SET context=json_set(context,'$.input',json(?2)) WHERE id=?1",
            params![turn, serde_json::to_string(&input)?],
        )?;
        turn
    } else {
        crate::conversations::execution::accept_opening(
            db,
            registry,
            &captured,
            conversation,
            "Begin the conversation.",
            &opening,
        )?
    };
    db.execute(
        "INSERT INTO conversation_openings(conversation_id,opening,turn_id) VALUES(?1,?2,?3)",
        params![conversation, serde_json::to_string(&opening)?, turn],
    )?;
    crate::learning::effort::bot::steering(db, conversation, &configuration)?;
    Ok(turn)
}
