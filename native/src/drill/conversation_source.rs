//! Local extraction of exact sentence spans from current conversation revisions.
use super::{generation::DrillGenerationInput, previews::*};
use crate::{
    configuration::LanguageContext, language::reading::ReadingScope, model::*,
    storage::store::Store,
};
use rusqlite::{Connection, OptionalExtension, params};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use ts_rs::TS;
use unicode_segmentation::UnicodeSegmentation;

#[derive(Deserialize, Serialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ConversationDrillInput {
    pub scope: ReadingScope,
    pub cursor: Option<String>,
    pub limit: u32,
}
#[derive(Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct ConversationDrillPage {
    pub preview: DrillGenerationPreview,
    pub next_cursor: Option<String>,
}
fn revision(text: &str) -> String {
    format!("{:x}", Sha256::digest(text.as_bytes()))
}

pub(crate) fn validate(
    db: &Connection,
    candidate: &DrillCandidate,
    scope: &LanguageContext,
) -> Result<()> {
    let DrillSource::Conversation {
        source_ref,
        revision: expected,
    } = &candidate.source
    else {
        return Ok(());
    };
    let row:Option<(String,String)>=db.query_row("SELECT m.text,t.context FROM messages m JOIN turns t ON t.id=m.turn_id JOIN conversations c ON c.id=m.conversation_id JOIN contacts p ON p.id=c.contact_id WHERE m.id=?1 AND m.turn_id=?2 AND m.conversation_id=?3 AND c.archived=0 AND p.archived=0 AND NOT EXISTS(SELECT 1 FROM turns n WHERE n.replaces_turn_id=t.id)",params![source_ref.message_id,source_ref.turn_id,source_ref.conversation_id],|r|Ok((r.get(0)?,r.get(1)?))).optional()?;
    let valid = row.is_some_and(|(text, context)| {
        let scope_saved: serde_json::Value = serde_json::from_str(&context).unwrap_or_default();
        revision(&text) == *expected
            && text.get(source_ref.start_byte as usize..source_ref.end_byte as usize)
                == Some(candidate.text.as_str())
            && scope_saved["languageContext"]["language_id"] == scope.language_id
            && scope_saved["languageContext"]["variety_id"] == scope.variety_id
    });
    if !valid {
        return Err(super::invalid(
            "The conversation source changed or was removed. Request fresh candidates.",
        ));
    }
    Ok(())
}
impl Store {
    pub fn conversation_drill_candidates(
        &mut self,
        input: ConversationDrillInput,
    ) -> Result<ConversationDrillPage> {
        if !(1..=50).contains(&input.limit) {
            return Err(super::invalid(
                "Choose 1–50 conversation candidates per page.",
            ));
        }
        let scope = self.config.resolve_pair(
            &input.scope.language,
            input.scope.variety.as_deref(),
            &input.scope.explanation,
            input.scope.explanation_variety.as_deref(),
        )?;
        let (before, offset) = if let Some(cursor) = input.cursor {
            if cursor.len() > 512 {
                return Err(super::invalid("Invalid conversation cursor."));
            }
            let (v, language, variety, before, offset): (u8, String, String, i64, usize) =
                serde_json::from_str(&cursor)
                    .map_err(|_| super::invalid("Invalid conversation cursor."))?;
            if v != 1 || language != scope.language_id || variety != scope.variety_id || before < 1
            {
                return Err(super::invalid(
                    "This cursor belongs to another language scope.",
                ));
            }
            (before, offset)
        } else {
            (i64::MAX, 0)
        };
        // Bound scans too: a page can be empty and still provide a continuation.
        let rows=self.connection.prepare("SELECT m.rowid,m.id,m.conversation_id,m.turn_id,m.role,m.text FROM messages m JOIN turns t ON t.id=m.turn_id JOIN conversations c ON c.id=m.conversation_id JOIN contacts p ON p.id=c.contact_id WHERE m.rowid<=?1 AND c.language_id=?2 AND json_extract(t.context,'$.languageContext.variety_id')=?3 AND c.archived=0 AND p.archived=0 AND NOT EXISTS(SELECT 1 FROM turns n WHERE n.replaces_turn_id=t.id) AND EXISTS(SELECT 1 FROM turn_execution_owners o WHERE o.turn_id=t.id AND o.channel IN ('persona_reply','persona_opening')) ORDER BY m.rowid DESC LIMIT 50")?
            .query_map(params![before,scope.language_id,scope.variety_id],|r|Ok((r.get::<_,i64>(0)?,r.get::<_,String>(1)?,r.get::<_,String>(2)?,r.get::<_,String>(3)?,r.get::<_,String>(4)?,r.get::<_,String>(5)?)))?.collect::<rusqlite::Result<Vec<_>>>()?;
        let count = rows.len();
        let mut candidates = Vec::new();
        let mut next = None;
        let mut seen = std::collections::HashSet::new();
        'messages: for (row, id, conversation, turn, role, text) in rows {
            for (start, span) in text.split_sentence_bound_indices() {
                let end = start + span.len();
                if row == before && end <= offset {
                    continue;
                }
                next = Some((row, end));
                // Trimming only changes the selected span; every retained byte
                // is still an exact substring, never a corrected paraphrase.
                let phrase = span.trim();
                if !valid_text(phrase)
                    || !seen.insert(phrase.to_owned())
                    || duplicate(&self.connection, &scope, phrase)?
                {
                    continue;
                }
                let start = start + (span.len() - span.trim_start().len());
                candidates.push(DrillCandidate {
                    candidate_id: uuid::Uuid::new_v4().to_string(),
                    text: phrase.into(),
                    translation: None,
                    reported: DrillReportedLabels {
                        difficulty: None,
                        tags: vec![],
                    },
                    verified: DrillVerifiedProperties {
                        scope_matches_request: true,
                        length_ok: true,
                        non_empty: true,
                        duplicate: false,
                    },
                    source: DrillSource::Conversation {
                        revision: revision(&text),
                        source_ref: DrillConversationRef {
                            conversation_id: conversation.clone(),
                            message_id: id.clone(),
                            turn_id: turn.clone(),
                            role: role.clone(),
                            start_byte: start as u32,
                            end_byte: (start + phrase.len()) as u32,
                        },
                    },
                });
                if candidates.len() == input.limit as usize {
                    break 'messages;
                }
            }
            next = (row > 1).then_some((row - 1, 0));
        }
        let next_cursor = if candidates.len() == input.limit as usize || count == 50 {
            next.map(|(row, offset)| {
                serde_json::to_string(&(1, &scope.language_id, &scope.variety_id, row, offset))
            })
            .transpose()?
        } else {
            None
        };
        let id = uuid::Uuid::new_v4().to_string();
        let captured = PreviewInput {
            skill_focus: None,
            scope: ReadingScope {
                language: scope.language_id,
                variety: Some(scope.variety_id),
                explanation: scope.explanation_language_id,
                explanation_variety: Some(scope.explanation_variety_id),
            },
            count: input.limit,
            requested: None::<DrillGenerationInput>,
        };
        let tx = self.connection.transaction()?;
        reserve(&tx, &id, "conversation", &captured, None)?;
        persist(&tx, &id, &candidates)?;
        tx.commit()?;
        Ok(ConversationDrillPage {
            preview: self.drill_preview(&id)?,
            next_cursor,
        })
    }
}

#[cfg(test)]
#[path = "conversation_source_tests.rs"]
mod tests;
