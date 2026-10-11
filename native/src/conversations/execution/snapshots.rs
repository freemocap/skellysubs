use super::*;

impl Store {
    /// Turns older than `before` (a turn ID), newest first. Independent of
    /// message paging, so history reaches every recorded turn.
    pub fn turn_history(
        &self,
        conversation: &str,
        before: Option<&str>,
        limit: u32,
    ) -> Result<crate::model::TurnHistoryPage> {
        let db = &self.connection;
        if !db.query_row(
            "SELECT EXISTS(SELECT 1 FROM conversations WHERE id=?1)",
            [conversation],
            |r| r.get::<_, bool>(0),
        )? {
            return Err(AppError::new(
                ErrorCode::NotFound,
                "Conversation no longer exists.",
            ));
        }
        let limit = limit.clamp(1, 100);
        let anchor: Option<i64> = match before {
            None => None,
            Some(turn) => Some(
                db.query_row(
                    "SELECT rowid FROM turns WHERE id=?1 AND conversation_id=?2",
                    params![turn, conversation],
                    |r| r.get(0),
                )
                .optional()?
                .ok_or_else(|| AppError::new(ErrorCode::NotFound, "Turn no longer exists."))?,
            ),
        };
        let mut rows = db
            .prepare("SELECT id,state,paused,route,refusal_hold FROM turns WHERE conversation_id=?1 AND (?2 IS NULL OR rowid<?2) ORDER BY rowid DESC LIMIT ?3")?
            .query_map(params![conversation, anchor, limit + 1], |r| {
                Ok((
                    r.get::<_, String>(0)?,
                    r.get::<_, String>(1)?,
                    r.get::<_, bool>(2)?,
                    r.get::<_, String>(3)?,
                    r.get::<_, Option<String>>(4)?,
                ))
            })?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        let has_older = rows.len() > limit as usize;
        rows.truncate(limit as usize);
        let turns = rows
            .into_iter()
            .map(|(id, state, paused, route, hold)| {
                self.turn_view(db, id, state, paused, route, hold)
            })
            .collect::<Result<Vec<_>>>()?;
        Ok(crate::model::TurnHistoryPage { turns, has_older })
    }

    pub fn conversation_snapshot(
        &self,
        conversation: &str,
        before: Option<i32>,
    ) -> Result<ConversationSnapshot> {
        self.conversation_snapshot_receipts(conversation, before, true)
    }

    pub(crate) fn conversation_snapshot_receipts(
        &self,
        conversation: &str,
        before: Option<i32>,
        include_receipts: bool,
    ) -> Result<ConversationSnapshot> {
        let db = &self.connection;
        if !db.query_row(
            "SELECT EXISTS(SELECT 1 FROM conversations WHERE id=?1)",
            [conversation],
            |r| r.get::<_, bool>(0),
        )? {
            return Err(AppError::new(
                ErrorCode::NotFound,
                "Conversation no longer exists.",
            ));
        }
        let mut messages=db.prepare("SELECT id,sequence,role,text,created_at,turn_id,(SELECT replaces_turn_id FROM turns WHERE id=m.turn_id),(SELECT id FROM turns WHERE replaces_turn_id=m.turn_id) FROM messages m WHERE conversation_id=?1 AND EXISTS(SELECT 1 FROM turn_execution_owners o WHERE o.turn_id=m.turn_id AND o.channel IN ('persona_reply','persona_opening')) AND sequence<?2 ORDER BY sequence DESC LIMIT 100")?.query_map(params![conversation,before.unwrap_or(i32::MAX)],|r|Ok(ChatMessage{assessment_state:None,assessment_error:None,guide_context:None,feedback_context:None,reply_brief:None,brief_state:None,brief_error:None,reading_scope:None,conversation_feedback:None,reply_assistance:None,reply_explanations:None,explanations_state:None,explanations_error:None,reaction:None,reaction_error:None,coach_decision:None,turn_id:r.get(5)?,replaces_turn_id:r.get(6)?,replaced_by:r.get(7)?,feedback_state:None,feedback_error:None,feedback:None,suggested_replies:None,suggestions_state:None,suggestions_error:None,gloss_error:None,word_gloss:None,gloss_state:None,gloss_operation_id:None,translation_state:None,translation:None,id:r.get(0)?,sequence:r.get(1)?,role:r.get(2)?,text:r.get(3)?,created_at:r.get(4)?}))?.collect::<rusqlite::Result<Vec<_>>>()?;
        messages.reverse();
        let mut contexts = std::collections::HashMap::<String, serde_json::Value>::new();
        for message in &mut messages {
            let captured = match contexts.entry(message.turn_id.clone()) {
                std::collections::hash_map::Entry::Occupied(entry) => entry.into_mut(),
                std::collections::hash_map::Entry::Vacant(entry) => {
                    let raw: String = db.query_row(
                        "SELECT context FROM turns WHERE id=?1",
                        [&message.turn_id],
                        |r| r.get(0),
                    )?;
                    entry.insert(serde_json::from_str(&raw)?)
                }
            };
            let language: crate::configuration::LanguageContext =
                serde_json::from_value(captured["languageContext"].clone())?;
            message.reading_scope = Some(crate::language::reading::ReadingScope {
                language: language.language_id,
                variety: Some(language.variety_id),
                explanation: language.explanation_language_id,
                explanation_variety: Some(language.explanation_variety_id),
            });
            if message.role == "user" {
                let (captured, feedback) = crate::conversations::assessments::view(
                    db,
                    &message.turn_id,
                    &self.graph_runtime,
                )?;
                message.feedback_context = captured
                    .get("feedbackContext")
                    .and_then(|v| v.as_str())
                    .map(str::to_owned);
                message.conversation_feedback = feedback;
                message.feedback = crate::learning::coaching::coach_policy::view(&captured)?;
                message.coach_decision = captured
                    .get("coachDecision")
                    .cloned()
                    .map(serde_json::from_value)
                    .transpose()?;
            } else {
                message.reaction = context_field(captured, "partnerReaction")?;
                message.reaction_error = context_field(captured, "skill_assessmentError")?;
                message.reply_brief = captured
                    .get("reply_brief")
                    .cloned()
                    .map(serde_json::from_value)
                    .transpose()?;
                message.reply_assistance = captured
                    .get("reply_assistance")
                    .cloned()
                    .map(serde_json::from_value)
                    .transpose()?;
                message.reply_explanations = captured
                    .get("reply_explanations")
                    .cloned()
                    .map(serde_json::from_value)
                    .transpose()?;
                message.suggested_replies = context_field(captured, "coachReplies")?;
            }

            message.word_gloss = context_field(
                captured,
                if message.role == "user" {
                    "userWordGloss"
                } else {
                    "wordGloss"
                },
            )?;
            message.translation = context_field(
                captured,
                if message.role == "user" {
                    "userTranslation"
                } else {
                    "translation"
                },
            )?;
        }
        self.graph_runtime.message_statuses(db, &mut messages)?;
        let first = messages.first().map(|m| m.sequence).unwrap_or(0);
        let has_older = db.query_row(
            "SELECT EXISTS(SELECT 1 FROM messages m WHERE conversation_id=?1 AND EXISTS(SELECT 1 FROM turn_execution_owners o WHERE o.turn_id=m.turn_id AND o.channel IN ('persona_reply','persona_opening')) AND sequence<?2)",
            params![conversation, first],
            |r| r.get(0),
        )?;
        // Older message pages require their owning execution state too. The union
        // stays bounded by this page's 100 messages plus the latest 50 turns.
        let page_turns = serde_json::to_string(
            &messages
                .iter()
                .map(|message| &message.turn_id)
                .collect::<Vec<_>>(),
        )?;
        let rows=db.prepare("SELECT id,state,paused,route,refusal_hold FROM turns WHERE conversation_id=?1 AND (id IN (SELECT id FROM turns WHERE conversation_id=?1 ORDER BY rowid DESC LIMIT 50) OR id IN (SELECT value FROM json_each(?2))) ORDER BY rowid DESC")?.query_map(params![conversation,page_turns],|r|Ok((r.get::<_,String>(0)?,r.get::<_,String>(1)?,r.get::<_,bool>(2)?,r.get::<_,String>(3)?,r.get::<_,Option<String>>(4)?)))?.collect::<rusqlite::Result<Vec<_>>>()?;
        let turns = rows
            .into_iter()
            .map(|(id, state, paused, route, hold)| {
                self.turn_view(db, id, state, paused, route, hold)
            })
            .collect::<Result<Vec<_>>>()?;
        let mut coach_messages=db.prepare("SELECT id,sequence,role,text,created_at,turn_id,(SELECT replaces_turn_id FROM turns WHERE id=m.turn_id),(SELECT id FROM turns WHERE replaces_turn_id=m.turn_id) FROM messages m WHERE conversation_id=?1 AND EXISTS(SELECT 1 FROM turn_execution_owners o WHERE o.turn_id=m.turn_id AND o.channel='coach') ORDER BY sequence DESC LIMIT 100")?.query_map([conversation],|r|Ok(ChatMessage{assessment_state:None,assessment_error:None,guide_context:None,feedback_context:None,reply_brief:None,brief_state:None,brief_error:None,reading_scope:None,conversation_feedback:None,reply_assistance:None,reply_explanations:None,explanations_state:None,explanations_error:None,reaction:None,reaction_error:None,coach_decision:None,turn_id:r.get(5)?,replaces_turn_id:r.get(6)?,replaced_by:r.get(7)?,feedback_state:None,feedback_error:None,feedback:None,suggested_replies:None,suggestions_state:None,suggestions_error:None,gloss_error:None,word_gloss:None,gloss_state:None,gloss_operation_id:None,translation_state:None,translation:None,id:r.get(0)?,sequence:r.get(1)?,role:r.get(2)?,text:r.get(3)?,created_at:r.get(4)?}))?.collect::<rusqlite::Result<Vec<_>>>()?;
        coach_messages.reverse();
        for message in &mut coach_messages {
            if message.role == "user" {
                let raw: Option<String> = db.query_row(
                    "SELECT json_extract(context,'$.guideContext') FROM turns WHERE id=?1",
                    [&message.turn_id],
                    |r| r.get(0),
                )?;
                message.guide_context = raw.map(|raw| serde_json::from_str(&raw)).transpose()?;
            }
        }
        let snapshot = self.snapshot()?;
        // Starter content is named in the languages of THIS conversation, not in
        // the learner's interface locale: the explanation language is a
        // per-conversation setting.
        let owner = snapshot
            .conversations
            .iter()
            .find(|c| c.id == conversation)
            .ok_or_else(|| AppError::new(ErrorCode::NotFound, "Conversation no longer exists."))?;
        Ok(ConversationSnapshot {
            phrase_seed: db.query_row("SELECT json_extract(t.context,'$.phraseSeed.text') FROM conversation_openings o JOIN turns t ON t.id=o.turn_id WHERE o.conversation_id=?1",[conversation],|r|r.get(0)).optional()?.flatten(),
            topic_choices: crate::conversations::openers::choices(
                &self.config,
                &owner.language_id,
                Some(&owner.settings.variety_id),
                &owner.settings.explanation_language,
            )?,
            starter_greeting: self
                .config
                .starter_greeting(&owner.language_id, Some(&owner.settings.variety_id))?,
            opening: crate::conversations::openers::selected(db, conversation)?,
            revision_suffix_counts: crate::conversations::revision::suffix_counts(
                db,
                conversation,
                &messages,
            )?,
            transcription_attempts: crate::speech::recording::transcription::views_using(
                db,
                &crate::speech::recording::owner::RecordingOwner::Conversation(conversation.into()),
                |id| if include_receipts { crate::ai::results::receipt_for_consumer(db,id) } else { Ok(None) },
            )?,
            coach_messages,
            conversation_id: conversation.into(),
            session_id: self.session_id.clone(),
            revision: db.query_row("SELECT revision FROM metadata", [], |r| r.get(0))?,
            messages,
            turns,
            connection: config(db)?,
            has_older,
        })
    }
}

/// Match JSON extraction's missing/null behavior while retaining typed validation.
fn context_field<T: serde::de::DeserializeOwned>(
    context: &serde_json::Value,
    key: &str,
) -> Result<Option<T>> {
    context
        .get(key)
        .filter(|value| !value.is_null())
        .cloned()
        .map(serde_json::from_value)
        .transpose()
        .map_err(Into::into)
}

/// A turn's graph inspection and source-bound product requests.
/// The conversation snapshot and turn history share this builder, so the AI
/// View never sees two descriptions of the same turn.
impl Store {
    #[allow(clippy::too_many_arguments)]
    fn turn_view(
        &self,
        db: &Connection,
        id: String,
        state: String,
        paused: bool,
        route: String,
        hold: Option<String>,
    ) -> Result<TurnView> {
        let speech = self.graph_runtime.speech_request_view(db, &id)?;
        let owner: Option<String> = db.query_row("SELECT t.conversation_id FROM turns t JOIN turn_execution_owners o ON o.turn_id=t.id WHERE t.id=?1 AND o.executor='graph'",[&id],|r|r.get(0)).optional()?;
        let native_graph = owner
            .map(|conversation| self.graph_runtime.inspection(db, &conversation, &id))
            .transpose()?
            .flatten();
        let native_surfaces = native_graph
            .as_ref()
            .map(|graph| super::graph_publication::surfaces(db, &id, graph))
            .transpose()?;
        let (replaces_turn_id, replaced_by) = db.query_row("SELECT replaces_turn_id,(SELECT id FROM turns child WHERE child.replaces_turn_id=turns.id) FROM turns WHERE id=?1", [&id], |r| Ok((r.get(0)?, r.get(1)?)))?;
        let recording_ids = db.query_row("SELECT coalesce(json_extract(context,'$.input.recording_ids'),'null') FROM turns WHERE id=?1", [&id], |r|r.get::<_,String>(0))?;
        Ok(TurnView {
            recording_ids: serde_json::from_str(&recording_ids)?,
            native_surfaces,
            award_sources: Some(db.prepare("SELECT award_source FROM conversation_graph_effects WHERE turn_id=?1 ORDER BY rowid")?.query_map([&id], |row| row.get(0))?.collect::<rusqlite::Result<Vec<String>>>()?),
            native_execution_available: {
                let owner: Option<String> = db.query_row("SELECT t.conversation_id FROM turns t JOIN turn_execution_owners o ON o.turn_id=t.id WHERE t.id=?1 AND o.executor='graph'",[&id],|r|r.get(0)).optional()?;
                owner
                    .map(|conversation| self.graph_runtime.owns(db, &conversation, &id))
                    .transpose()?
            },
            native_response: {
                let owner: Option<String> = db.query_row("SELECT t.conversation_id FROM turns t JOIN turn_execution_owners o ON o.turn_id=t.id WHERE t.id=?1 AND o.executor='graph'",[&id],|r|r.get(0)).optional()?;
                owner
                    .map(|conversation| self.graph_runtime.response(db, &conversation, &id))
                    .transpose()?
                    .flatten()
            },
            channel: db
                .query_row(
                    "SELECT channel FROM turn_execution_owners WHERE turn_id=?1",
                    [&id],
                    |r| r.get(0),
                )
                .optional()?,
            native_preview: {
                let owner: Option<String> = db.query_row("SELECT t.conversation_id FROM turns t JOIN turn_execution_owners o ON o.turn_id=t.id WHERE t.id=?1 AND o.executor='graph'",[&id],|r|r.get(0)).optional()?;
                owner
                    .map(|conversation| self.graph_runtime.preview(db, &conversation, &id))
                    .transpose()?
                    .flatten()
            },
            native_graph,
            replaces_turn_id,
            replaced_by,
            route: ConnectionRoute::parse(&route)?,
            id: id.clone(),
            state,
            paused,
            hold: hold.map(|json| serde_json::from_str(&json)).transpose()?,
            speech,
        })
    }
}
