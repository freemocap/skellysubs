use super::*;

impl Handlers<'_> {
    pub(super) fn start_conversation(
        &mut self,
        conversation_id: String,
        configuration: crate::conversations::direction::ConversationStartConfig,
        message: Option<String>,
        input: Option<crate::learning::coaching::InputEvidence>,
        expected_revision: i32,
    ) -> Result<String> {
        let learner_message = match (message, input) {
            (Some(text), Some(input)) => Some((text, input)),
            (None, None) => None,
            _ => {
                return Err(AppError::new(
                    ErrorCode::Validation,
                    "Learner messages require input provenance; partner openings must not include it.",
                ));
            }
        };
        let kind = if learner_message.is_some() {
            crate::conversations::execution::context::Kind::Reply
        } else {
            crate::conversations::execution::context::Kind::Opening
        };
        let id = crate::conversations::openers::accept_native(
            self.tx,
            self.snapshot,
            self.config,
            &conversation_id,
            configuration,
            learner_message,
            expected_revision,
        )?;
        self.conversation_scope = Some(conversation_id);
        self.graph_admission = Some((
            id.clone(),
            crate::conversations::execution::graph_runtime::Admission::Partner(kind),
        ));
        Ok(id)
    }

    pub(super) fn revise_turn(
        &mut self,
        conversation_id: String,
        turn_id: String,
        text: String,
        input: crate::learning::coaching::InputEvidence,
        expected_revision: i32,
    ) -> Result<String> {
        let result = crate::conversations::revision::accept(
            self.tx,
            self.config,
            self.snapshot,
            &conversation_id,
            &turn_id,
            &text,
            input,
            expected_revision,
        )?;
        self.conversation_scope = Some(conversation_id);
        self.graph_admission = Some((
            result.clone(),
            crate::conversations::execution::graph_runtime::Admission::Partner(
                crate::conversations::execution::context::Kind::Reply,
            ),
        ));
        Ok(result)
    }

    pub(super) fn send_message(
        &mut self,
        input: crate::learning::coaching::InputEvidence,
        conversation_id: String,
        text: String,
        expected_revision: i32,
    ) -> Result<String> {
        let turn_id = crate::conversations::execution::capture_native_send(
            self.tx,
            self.config,
            self.snapshot,
            &conversation_id,
            &text,
            expected_revision,
        )?;
        crate::speech::recording::transcription::validate_sources(
            self.tx,
            &conversation_id,
            input.recording_ids.as_deref().unwrap_or_default(),
        )?;
        if !matches!(input.modality.as_str(), "text" | "speech_transcript") {
            return Err(AppError::new(
                ErrorCode::Validation,
                "Invalid input modality.",
            ));
        }
        self.tx.execute(
            "UPDATE turns SET context=json_set(context,'$.input',json(?2)) WHERE id=?1",
            params![turn_id, serde_json::to_string(&input)?],
        )?;
        self.conversation_scope = Some(conversation_id);
        self.graph_admission = Some((
            turn_id.clone(),
            crate::conversations::execution::graph_runtime::Admission::Partner(
                crate::conversations::execution::context::Kind::Reply,
            ),
        ));
        Ok(turn_id)
    }

    pub(super) fn create_conversation(
        &mut self,
        contact_id: String,
        title: String,
    ) -> Result<String> {
        short_text(&title, "Conversation title", 100)?;
        let contact = self
            .snapshot
            .contacts
            .iter()
            .find(|r| r.id == contact_id)
            .ok_or_else(missing)?;
        if contact.archived {
            return Err(AppError::new(
                ErrorCode::Validation,
                "Restore this persona before creating a conversation.",
            ));
        }
        let persona = self
            .snapshot
            .personas
            .iter()
            .find(|p| p.id == contact.persona_id)
            .ok_or_else(missing)?;
        let defaults = self
            .config
            .preference_defaults(&persona.language_id, &self.snapshot.learner.preferences)?;
        let mut settings = self
            .snapshot
            .conversations
            .iter()
            .filter(|c| c.contact_id == contact_id)
            .max_by(|a, b| a.last_used.cmp(&b.last_used).then(a.id.cmp(&b.id)))
            .map(|c| c.settings.clone())
            .unwrap_or_else(|| defaults.clone());
        settings.direction = Default::default();
        settings.variety_id = defaults.variety_id;
        settings.explanation_language = defaults.explanation_language;
        settings.explanation_variety_id = defaults.explanation_variety_id;
        let conversation_id = create_conversation(
            self.tx,
            &contact_id,
            &persona.language_id,
            title.trim(),
            &settings,
        )?;
        self.persona_scope = Some(persona.id.clone());
        self.conversation_scope = Some(conversation_id.clone());
        Ok(conversation_id)
    }

    pub(super) fn open_conversation(&mut self, conversation_id: String) -> Result<String> {
        let conversation = self
            .snapshot
            .conversations
            .iter()
            .find(|c| c.id == conversation_id)
            .ok_or_else(missing)?;
        self.tx.execute(
            "UPDATE conversations SET last_used=MAX(CAST((julianday('now')-2440587.5)*86400000 AS INTEGER),COALESCE((SELECT MAX(last_used) FROM conversations),0)+1) WHERE id=?1",
            params![conversation_id],
        )?;
        self.conversation_scope = Some(conversation.id.clone());
        Ok(conversation_id)
    }

    pub(super) fn update_conversation(
        &mut self,
        conversation_id: String,
        expected_revision: i32,
        title: String,
        archived: bool,
    ) -> Result<String> {
        short_text(&title, "Conversation title", 100)?;
        let conversation = self
            .snapshot
            .conversations
            .iter()
            .find(|c| c.id == conversation_id)
            .ok_or_else(missing)?;
        check_revision(conversation.revision, expected_revision)?;
        self.tx.execute(
            "UPDATE conversations SET title=?1,archived=?2,revision=revision+1 WHERE id=?3",
            params![title.trim(), archived, conversation_id],
        )?;
        self.conversation_scope = Some(conversation_id.clone());
        Ok(conversation_id)
    }

    pub(super) fn update_settings(
        &mut self,
        conversation_id: String,
        expected_revision: i32,
        settings: PracticeSettings,
    ) -> Result<String> {
        let conversation = self
            .snapshot
            .conversations
            .iter()
            .find(|c| c.id == conversation_id)
            .ok_or_else(missing)?;
        check_revision(conversation.settings_revision, expected_revision)?;
        self.config
            .validate_settings(&conversation.language_id, &settings)?;
        self.tx.execute("UPDATE conversation_settings SET settings=?1,revision=revision+1 WHERE conversation_id=?2", params![serde_json::to_string(&settings)?, conversation_id])?;
        self.tx.execute(
            "UPDATE conversations SET revision=revision+1 WHERE id=?1",
            [&conversation_id],
        )?;
        self.conversation_scope = Some(conversation_id.clone());
        Ok(conversation_id)
    }

    pub(super) fn delete_conversation(
        &mut self,
        conversation_id: String,
        expected_revision: i32,
    ) -> Result<String> {
        let conversation = self
            .snapshot
            .conversations
            .iter()
            .find(|c| c.id == conversation_id)
            .ok_or_else(missing)?;
        check_revision(conversation.revision, expected_revision)?;
        self.tx
            .execute("DELETE FROM conversations WHERE id=?1", [&conversation_id])?;
        Ok(conversation_id)
    }
}
