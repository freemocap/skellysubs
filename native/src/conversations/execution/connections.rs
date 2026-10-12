use super::*;

fn valid_model_id(model: &str) -> bool {
    !model.is_empty()
        && model.len() <= 256
        && !model.chars().any(|c| c.is_whitespace() || c.is_control())
}

pub(crate) fn invalidate(db: &Connection, _revoked: Option<ConnectionRoute>) -> Result<()> {
    db.execute("UPDATE turns SET state=CASE WHEN EXISTS(SELECT 1 FROM messages m WHERE m.turn_id=turns.id AND m.role='assistant') THEN 'succeeded' ELSE 'invalidated' END WHERE state IN ('pending','assisting')", [])?;
    Ok(())
}

impl Store {
    pub fn note_refusal(
        &mut self,
        target: &crate::ai::connections::access::ResolvedTarget,
        error: &AppError,
    ) -> Result<()> {
        let tx = self.connection.transaction()?;
        pause_related(&tx, target, error)?;
        tx.commit()?;
        Ok(())
    }

    pub fn connection_config(&self) -> Result<ConnectionConfig> {
        config(&self.connection)
    }

    pub fn reserve_credential(&mut self, id: &str) -> Result<()> {
        self.connection
            .execute("INSERT INTO credential_cleanup VALUES(?1)", [id])?;
        self.credential_writes.insert(id.to_owned());
        Ok(())
    }

    pub fn claim_credential_cleanup(&mut self) -> Result<Option<String>> {
        let ids = self
            .connection
            .prepare("SELECT id FROM credential_cleanup")?
            .query_map([], |r| r.get::<_, String>(0))?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        for id in ids {
            if !self.credential_writes.contains(&id) {
                self.credential_writes.insert(id.clone());
                return Ok(Some(id));
            }
        }
        Ok(None)
    }

    pub fn finish_credential_cleanup(&mut self, id: &str, removed: bool) -> Result<()> {
        self.credential_writes.remove(id);
        if removed {
            self.connection
                .execute("DELETE FROM credential_cleanup WHERE id=?1", [id])?;
        }
        Ok(())
    }

    pub fn set_models(
        &mut self,
        expected: i32,
        standard: &str,
        fast: &str,
        audio: &AudioSettings,
        assessment_adapter: AssessmentAdapter,
    ) -> Result<()> {
        if ![
            standard,
            fast,
            &audio.transcription.model,
            &audio.speech.model,
        ]
        .iter()
        .all(|model| valid_model_id(model))
        {
            return Err(fail(
                "Provide explicit valid model IDs for Standard, Fast, Transcription and Speech.",
            ));
        }
        let tx = self.connection.transaction()?;
        if config(&tx)?.revision != expected {
            return Err(AppError::new(
                ErrorCode::Conflict,
                "AI settings changed. Reload before saving models.",
            ));
        }
        let previous = config(&tx)?;
        let routing_changed = previous.standard_model != standard
            || previous.fast_model != fast
            || serde_json::to_value(&previous.audio)? != serde_json::to_value(audio)?;
        tx.execute("UPDATE ai_config SET revision=revision+1,standard_model=?1,fast_model=?2,audio_settings=?3,assessment_adapter=?4", params![standard,fast,serde_json::to_string(audio)?,assessment_adapter.label()])?;
        // Adapter-only changes apply to future work; captured operations keep their strategy.
        if routing_changed {
            invalidate(&tx, None)?;
        }
        bump(&tx)?;
        tx.commit()?;
        Ok(())
    }

    pub fn hosted_credential(&self) -> Result<Option<String>> {
        Ok(self
            .connection
            .query_row("SELECT hosted_credential_id FROM ai_config", [], |r| {
                r.get(0)
            })?)
    }

    pub fn set_hosted_connection(
        &mut self,
        expected: i32,
        credential: Option<&str>,
        email: &str,
    ) -> Result<()> {
        let tx = self.connection.transaction()?;
        if config(&tx)?.revision != expected {
            return Err(AppError::new(
                ErrorCode::Conflict,
                "AI settings changed. Reload before continuing.",
            ));
        }
        tx.execute("INSERT OR IGNORE INTO credential_cleanup SELECT hosted_credential_id FROM ai_config WHERE hosted_credential_id IS NOT NULL AND hosted_credential_id IS NOT ?1",[credential])?;
        if let Some(id) = credential {
            tx.execute("DELETE FROM credential_cleanup WHERE id=?1", [id])?;
        }
        tx.execute(
            "UPDATE ai_config SET revision=revision+1,hosted_credential_id=?1,hosted_email=?2",
            params![credential, email],
        )?;
        invalidate(&tx, Some(ConnectionRoute::Hosted))?;
        bump(&tx)?;
        tx.commit()?;
        Ok(())
    }

    pub fn select_route(&mut self, expected: i32, route: ConnectionRoute) -> Result<()> {
        let tx = self.connection.transaction()?;
        if config(&tx)?.revision != expected {
            return Err(AppError::new(
                ErrorCode::Conflict,
                "AI settings changed. Reload before switching.",
            ));
        }
        tx.execute(
            "UPDATE ai_config SET revision=revision+1,route=?1",
            [route.label()],
        )?;
        invalidate(&tx, None)?;
        bump(&tx)?;
        tx.commit()?;
        Ok(())
    }
}
