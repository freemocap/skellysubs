use super::*;

impl Handlers<'_> {
    pub(super) fn start_phrase_conversation(
        &mut self,
        source_message_id: String,
        phrase: String,
        contact_id: String,
        expected_revision: i32,
    ) -> Result<String> {
        check_revision(self.snapshot.revision, expected_revision)?;
        let source_id =
            crate::conversations::phrase_start::source(self.tx, &source_message_id, &phrase)?;
        let source = self
            .snapshot
            .conversations
            .iter()
            .find(|c| c.id == source_id)
            .ok_or_else(missing)?;
        let settings = source.settings.clone();
        let language = source.language_id.clone();
        let contact = self
            .snapshot
            .contacts
            .iter()
            .find(|c| c.id == contact_id)
            .ok_or_else(missing)?;
        let persona = self
            .snapshot
            .personas
            .iter()
            .find(|p| p.id == contact.persona_id)
            .ok_or_else(missing)?;
        if persona.language_id != language {
            return Err(AppError::new(
                ErrorCode::Validation,
                "Choose a partner for the source message's language.",
            ));
        }
        let conversation = self.create_conversation(contact_id, "Conversation".into())?;
        self.tx.execute(
            "UPDATE conversation_settings SET settings=?2 WHERE conversation_id=?1",
            params![conversation, serde_json::to_string(&settings)?],
        )?;
        let current = snapshot::read_snapshot(self.tx, &self.snapshot.session_id, self.config)?;
        let turn = crate::conversations::openers::accept_native(
            self.tx,
            &current,
            self.config,
            &conversation,
            crate::conversations::direction::ConversationStartConfig {
                prompt_editor: None,
                difficulty: settings.difficulty,
                variety_id: settings.variety_id,
                direction: crate::conversations::direction::ConversationDirection {
                    topic: None,
                    ..settings.direction
                },
            },
            None,
            current.revision,
        )?;
        crate::conversations::phrase_start::capture(
            self.tx,
            &turn,
            &source_message_id,
            &phrase,
            &self.config.conversation_prompt().phrase_opening,
        )?;
        self.graph_admission = Some((
            turn,
            crate::conversations::execution::graph_runtime::Admission::Partner(
                crate::conversations::execution::context::Kind::SeededOpening,
            ),
        ));
        Ok(conversation)
    }
}
