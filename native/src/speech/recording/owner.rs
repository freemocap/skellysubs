//! Who a recording belongs to. The caller names an owner; language, variety and
//! recognizer context are resolved and validated here, never trusted from IPC.
use crate::ai::audio::TranscriptionLanguage;
use crate::model::*;
use rusqlite::{Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use ts_rs::TS;

#[derive(Clone, Debug, Deserialize, Serialize, TS, PartialEq, Eq)]
#[serde(tag = "kind", content = "id", rename_all = "camelCase")]
pub enum RecordingOwner {
    Conversation(String),
    DrillItem(String),
}

/// What an owner contributes to one recording, read from the workspace.
pub struct OwnerScope {
    pub language_context: crate::configuration::LanguageContext,
    pub language: TranscriptionLanguage,
    /// Recognizer context: the text the learner is answering or repeating.
    pub context: Option<String>,
}

fn fault(message: impl Into<String>) -> AppError {
    AppError::new(ErrorCode::Provider, message)
}

impl RecordingOwner {
    pub fn id(&self) -> &str {
        match self {
            RecordingOwner::Conversation(id) | RecordingOwner::DrillItem(id) => id,
        }
    }
    /// The `(conversation_id, drill_item_id)` pair this owner writes; exactly
    /// one is set, which the receipt table also enforces.
    pub(crate) fn columns(&self) -> (Option<&str>, Option<&str>) {
        match self {
            RecordingOwner::Conversation(id) => (Some(id), None),
            RecordingOwner::DrillItem(id) => (None, Some(id)),
        }
    }
    /// Whether this owner still exists and can own a recording.
    pub(crate) fn available(&self, db: &Connection) -> Result<bool> {
        let sql = match self {
            RecordingOwner::Conversation(_) => {
                "SELECT EXISTS(SELECT 1 FROM conversations c JOIN contacts r ON r.id=c.contact_id WHERE c.id=?1 AND c.archived=0 AND r.archived=0)"
            }
            RecordingOwner::DrillItem(_) => {
                "SELECT EXISTS(SELECT 1 FROM drill_items WHERE id=?1 AND archived=0)"
            }
        };
        Ok(db.query_row(sql, [self.id()], |r| r.get(0))?)
    }
    /// The language the recognizer is told to expect, and the text this
    /// recording answers. Both come from the owner's own record.
    pub(crate) fn scope(&self, store: &crate::storage::store::Store) -> Result<OwnerScope> {
        let (language_id, variety_id, explanation, explanation_variety, context) = match self {
            RecordingOwner::Conversation(id) => {
                let snapshot = store.snapshot()?;
                let conversation = snapshot
                    .conversations
                    .iter()
                    .find(|c| &c.id == id && !c.archived)
                    .ok_or_else(|| fault("Conversation is unavailable."))?;
                // The partner message the learner is answering.
                let previous: Option<String> = store.connection.query_row(
                    "SELECT m.text FROM messages m JOIN turns t ON t.id=m.turn_id WHERE m.conversation_id=?1 AND m.role='assistant' AND NOT EXISTS(SELECT 1 FROM turns child WHERE child.replaces_turn_id=t.id) AND EXISTS(SELECT 1 FROM turn_execution_owners o WHERE o.turn_id=t.id AND o.channel IN ('persona_reply','persona_opening')) ORDER BY m.sequence DESC LIMIT 1",
                    [id], |row| row.get(0),
                ).optional()?;
                (
                    conversation.language_id.clone(),
                    conversation.settings.variety_id.clone(),
                    conversation.settings.explanation_language.clone(),
                    conversation.settings.explanation_variety_id.clone(),
                    previous,
                )
            }
            RecordingOwner::DrillItem(id) => {
                // The item's own text is what the learner is repeating.
                let item = store
                    .connection
                    .query_row(
                        "SELECT language_id,variety_id,explanation_language,explanation_variety_id,text FROM drill_items WHERE id=?1 AND archived=0",
                        [id],
                        |r| {
                            Ok((
                                r.get::<_, String>(0)?,
                                r.get::<_, String>(1)?,
                                r.get::<_, String>(2)?,
                                r.get::<_, String>(3)?,
                                r.get::<_, String>(4)?,
                            ))
                        },
                    )
                    .optional()?
                    .ok_or_else(|| fault("Drill item is unavailable."))?;
                (item.0, item.1, item.2, item.3, Some(item.4))
            }
        };
        let context_languages = store.config.resolve_pair(
            &language_id,
            Some(&variety_id),
            &explanation,
            Some(&explanation_variety),
        )?;
        Ok(OwnerScope {
            language_context: context_languages.clone(),
            language: TranscriptionLanguage {
                language_id,
                variety_id,
                language_tag: context_languages
                    .external_tags
                    .get("language_tag")
                    .cloned()
                    .ok_or_else(|| fault("The selected language has no language tag."))?,
            },
            context,
        })
    }
}

#[cfg(test)]
#[path = "owner_tests.rs"]
mod tests;
