use super::*;

impl Handlers<'_> {
    pub(super) fn start_skill_conversation(
        &mut self,
        source_id: String,
        language: String,
        variety: String,
        topic: crate::conversations::direction::TopicChoice,
        expected_revision: i32,
    ) -> Result<String> {
        check_revision(self.snapshot.revision, expected_revision)?;
        let source = self
            .snapshot
            .conversations
            .iter()
            .find(|c| c.id == source_id && !c.archived)
            .ok_or_else(missing)?;
        if source.language_id != language {
            return Err(AppError::new(
                ErrorCode::Conflict,
                "The selected conversation uses a different language.",
            ));
        }
        let mut settings = source.settings.clone();
        settings.variety_id = variety;
        settings.direction.time_reference = crate::conversations::direction::TimeReference::Any;
        settings.direction.topic = Some(topic);
        // Validate authored material before creating anything. Admission and opening
        // capture remain in this command's transaction, including rollback on failure.
        self.config.skill_conversation_focus(&language, &settings)?;
        let conversation =
            self.create_conversation(source.contact_id.clone(), "Conversation".into())?;
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
                direction: settings.direction,
            },
            None,
            current.revision,
        )?;
        self.graph_admission = Some((
            turn,
            crate::conversations::execution::graph_runtime::Admission::Partner(
                crate::conversations::execution::context::Kind::Opening,
            ),
        ));
        Ok(conversation)
    }
}
