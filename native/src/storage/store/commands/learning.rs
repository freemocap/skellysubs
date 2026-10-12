use super::*;

impl Handlers<'_> {
    pub(super) fn coach_control(
        &mut self,
        turn_id: String,
        control: crate::learning::coaching::CoachControl,
    ) -> Result<String> {
        let id = crate::learning::coaching::coach_policy::control(self.tx, &turn_id, control)?;
        self.conversation_scope = Some(self.tx.query_row(
            "SELECT conversation_id FROM turns WHERE id=?1",
            [&turn_id],
            |r| r.get(0),
        )?);
        Ok(id)
    }

    pub(super) fn ask_coach(
        &mut self,
        conversation_id: String,
        text: String,
        expected_revision: i32,
    ) -> Result<String> {
        let turn_id = crate::conversations::execution::accept_coach(
            self.tx,
            self.config,
            self.snapshot,
            &conversation_id,
            &text,
            expected_revision,
        )?;
        self.conversation_scope = Some(conversation_id);
        self.graph_admission = Some((
            turn_id.clone(),
            crate::conversations::execution::graph_runtime::Admission::Coach,
        ));
        Ok(turn_id)
    }

    pub(super) fn update_learner(
        &mut self,
        expected_revision: i32,
        name: String,
        preferences: Preferences,
    ) -> Result<String> {
        check_revision(self.snapshot.learner.revision, expected_revision)?;
        short_text(&name, "Learner name", 80)?;
        self.config.validate_preferences(&preferences)?;
        if !(crate::model::TEXT_SIZE_MIN..=crate::model::TEXT_SIZE_MAX)
            .contains(&preferences.text_size)
            || preferences.text_spacing > 12
        {
            return Err(AppError::new(
                ErrorCode::Validation,
                "Reading size or spacing is outside the supported range.",
            ));
        }
        self.tx.execute(
            "UPDATE learner SET name=?1,preferences=?2,revision=revision+1 WHERE id=?3",
            params![
                name.trim(),
                serde_json::to_string(&preferences)?,
                self.snapshot.learner.id
            ],
        )?;
        Ok(self.snapshot.learner.id.clone())
    }
}
