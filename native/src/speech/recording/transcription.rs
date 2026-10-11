//! Product recording receipts and publication. Reusable recognition belongs to shared inference results.
use crate::ai::connections::access::ResolvedTarget;
use crate::model::*;
use crate::speech::recording::owner::RecordingOwner;
use rusqlite::{Connection, params};

pub fn permitted(db: &Connection, owner: &RecordingOwner, target: &ResolvedTarget) -> Result<()> {
    let valid = owner.available(db)?;
    let current = crate::ai::connections::access::resolve(
        db,
        crate::ai::connections::access::Capability::Transcription,
    )
    .map_err(|mut error| {
        if error.code == ErrorCode::Validation {
            error.code = ErrorCode::Conflict;
        }
        error
    })?;
    if !valid
        || current.route != target.route
        || current.model
            != target
                .audio_resolution
                .as_ref()
                .map_or(target.model.as_str(), |r| r.requested_model.as_str())
        || target
            .audio_resolution
            .as_ref()
            .is_some_and(|r| r.model != target.model)
        || current.url != target.url
        || current.credential != target.credential
    {
        return Err(AppError::new(
            ErrorCode::Conflict,
            "The recording destination, credentials, or its owner are no longer available.",
        ));
    }
    Ok(())
}

pub fn begin(
    db: &Connection,
    id: &str,
    owner: &RecordingOwner,
    target: &ResolvedTarget,
) -> Result<()> {
    permitted(db, owner, target)?;
    if crate::ai::connections::configuration::config(db)?.paused {
        return Err(AppError::new(
            ErrorCode::AdmissionHeld,
            "AI execution is paused. This recording was not submitted.",
        ));
    }
    // Duplicate invocation cannot authorize another network request.
    let (conversation, drill_item) = owner.columns();
    db.execute("INSERT INTO transcription_attempts(id,conversation_id,drill_item_id,route,model,profile_revision,state) VALUES(?1,?2,?3,?4,?5,?6,'running')",
        params![id, conversation, drill_item, target.route.label(), target.model, target.revision])?;
    db.execute("UPDATE metadata SET revision=revision+1", [])?;
    Ok(())
}

pub fn finish(
    db: &Connection,
    id: &str,
    owner: &RecordingOwner,
    target: &ResolvedTarget,
    result: Result<String>,
) -> Result<Result<String>> {
    let result = match permitted(db, owner, target) {
        Ok(()) => result,
        Err(error) if error.code == ErrorCode::Conflict => Err(AppError::new(
            ErrorCode::UnknownOutcome,
            "Transcription stopped because its connection or its owner changed. Provider processing and cost may continue; no transcript was inserted.",
        )),
        Err(error) => return Err(error),
    };
    let (state, error) = match &result {
        Ok(_) => ("succeeded", None),
        Err(error) => (
            if error.code == ErrorCode::UnknownOutcome {
                "unknown"
            } else {
                "failed"
            },
            Some(error.message.as_str()),
        ),
    };
    let (conversation, drill_item) = owner.columns();
    let changed = db.execute("UPDATE transcription_attempts SET state=?2,error=?3,finished_at=strftime('%Y-%m-%dT%H:%M:%fZ','now') WHERE id=?1 AND conversation_id IS ?4 AND drill_item_id IS ?5 AND state='running'",
        params![id, state, error, conversation, drill_item])?;
    if changed != 1 {
        return Err(AppError::new(
            ErrorCode::Conflict,
            "Transcription receipt is no longer active. No transcript was inserted.",
        ));
    }
    db.execute("UPDATE metadata SET revision=revision+1", [])?;
    Ok(result)
}

/// A process restart cannot establish the outcome of an interrupted recording.
pub(crate) fn recover(db: &Connection) -> Result<()> {
    let tx = db.unchecked_transaction()?;
    let changed = tx.execute("UPDATE transcription_attempts SET state='unknown',error='Recording interrupted; provider outcome is unknown.',finished_at=strftime('%Y-%m-%dT%H:%M:%fZ','now') WHERE state='running'", [])?;
    if changed != 0 {
        tx.execute("UPDATE metadata SET revision=revision+1", [])?;
    }
    tx.commit()?;
    Ok(())
}

impl crate::storage::store::Store {
    #[cfg(test)]
    pub fn begin_transcription(
        &mut self,
        id: &str,
        owner: &RecordingOwner,
        target: &ResolvedTarget,
    ) -> Result<()> {
        self.begin_transcription_in_visit(id, owner, target, None)
    }
    #[cfg(test)]
    pub(crate) fn begin_transcription_in_visit(
        &mut self,
        id: &str,
        owner: &RecordingOwner,
        target: &ResolvedTarget,
        visit: Option<&str>,
    ) -> Result<()> {
        self.transcription_in_visit(id, owner, target, visit, false)
    }
    pub(crate) fn reserve_transcription_in_visit(
        &mut self,
        id: &str,
        owner: &RecordingOwner,
        target: &ResolvedTarget,
        visit: Option<&str>,
    ) -> Result<()> {
        self.transcription_in_visit(id, owner, target, visit, true)
    }
    fn transcription_in_visit(
        &mut self,
        id: &str,
        owner: &RecordingOwner,
        target: &ResolvedTarget,
        visit: Option<&str>,
        shared: bool,
    ) -> Result<()> {
        let tx = self.connection.transaction()?;
        begin(&tx, id, owner, target)?;
        if shared {
            tx.execute("UPDATE transcription_attempts SET diagnostics=json_object('sharedExecution',json('true')) WHERE id=?1", [id])?;
        }
        if let Some(visit) = visit {
            let valid = matches!(owner, RecordingOwner::DrillItem(_))
                && tx.query_row(
                    "SELECT EXISTS(SELECT 1 FROM drill_visits WHERE id=?1 AND drill_item_id=?2)",
                    params![visit, owner.id()],
                    |r| r.get::<_, bool>(0),
                )?;
            if !valid {
                return Err(AppError::new(
                    ErrorCode::Conflict,
                    "Recording visit does not belong to its item.",
                ));
            }
            tx.execute(
                "UPDATE transcription_attempts SET drill_visit_id=?2 WHERE id=?1",
                params![id, visit],
            )?;
        }
        tx.commit()?;
        Ok(())
    }
    #[cfg(test)]
    pub fn finish_transcription(
        &mut self,
        id: &str,
        owner: &RecordingOwner,
        target: &ResolvedTarget,
        result: Result<String>,
    ) -> Result<String> {
        self.finish_transcription_with_diagnostics(id, owner, target, result, None)
    }
    pub fn finish_transcription_with_diagnostics(
        &mut self,
        id: &str,
        owner: &RecordingOwner,
        target: &ResolvedTarget,
        result: Result<String>,
        diagnostics: Option<&serde_json::Value>,
    ) -> Result<String> {
        self.publish_transcription(id, owner, target, result, diagnostics, None)
    }

    pub(crate) fn publish_transcription(
        &mut self,
        id: &str,
        owner: &RecordingOwner,
        target: &ResolvedTarget,
        result: Result<String>,
        diagnostics: Option<&serde_json::Value>,
        wav: Option<&[u8]>,
    ) -> Result<String> {
        self.publish_recording_result(id, owner, target, result, diagnostics, wav, None)
    }

    #[allow(clippy::too_many_arguments)]
    pub(crate) fn publish_recording_result(
        &mut self,
        id: &str,
        owner: &RecordingOwner,
        target: &ResolvedTarget,
        result: Result<String>,
        diagnostics: Option<&serde_json::Value>,
        wav: Option<&[u8]>,
        evidence: Option<&crate::ai::audio::TranscriptionResult>,
    ) -> Result<String> {
        let tx = self.connection.transaction()?;
        let diagnostic = crate::diagnostics::response::retained(diagnostics, result.as_ref().err());
        let (conversation, drill_item) = owner.columns();
        tx.execute(
            "UPDATE transcription_attempts SET diagnostics=CASE WHEN json_extract(diagnostics,'$.sharedExecution')=1 THEN json_set(COALESCE(?2,'{}'),'$.sharedExecution',json('true')) ELSE ?2 END WHERE id=?1 AND conversation_id IS ?3 AND drill_item_id IS ?4",
            params![id, diagnostic, conversation, drill_item],
        )?;
        let result = finish(&tx, id, owner, target, result)?;
        if let (Ok(_), Some(wav), Some(evidence)) = (&result, wav, evidence) {
            super::results::save(&tx, id, wav, evidence)?;
        }
        if let (RecordingOwner::DrillItem(item), Ok(text), Some(wav)) = (owner, &result, wav) {
            let reliability = diagnostics
                .and_then(|value| value.get("drill_reliability"))
                .map(|value| serde_json::from_value(value.clone()))
                .transpose()?;
            crate::drill::stage_attempt_with_reliability(
                &tx,
                item,
                Some(id),
                text,
                Some(wav),
                reliability,
            )?;
        }
        if result.is_ok() && matches!(owner, RecordingOwner::DrillItem(_)) {
            crate::drill::retention::mark(&tx)?;
        }
        tx.commit()?;
        if result.is_ok() && matches!(owner, RecordingOwner::DrillItem(_)) {
            self.prune_drill_audio()?;
        }
        result
    }
}

/// One owner's recording receipts, newest first.
#[cfg(test)]
pub fn views(db: &Connection, owner: &RecordingOwner) -> Result<Vec<TranscriptionAttempt>> {
    views_using(db, owner, |id| {
        crate::ai::results::receipt_for_consumer(db, id)
    })
}
pub(crate) fn views_using(
    db: &Connection,
    owner: &RecordingOwner,
    mut receipt: impl FnMut(&str) -> Result<Option<serde_json::Value>>,
) -> Result<Vec<TranscriptionAttempt>> {
    let (conversation, drill_item) = owner.columns();
    let rows = db.prepare("SELECT id,route,model,state,started_at,finished_at,error,diagnostics FROM transcription_attempts WHERE conversation_id IS ?1 AND drill_item_id IS ?2 ORDER BY rowid DESC LIMIT 50")?
        .query_map(params![conversation, drill_item], |r| Ok((r.get::<_,String>(0)?,r.get::<_,String>(1)?,r.get::<_,String>(2)?,r.get::<_,String>(3)?,r.get::<_,String>(4)?,r.get::<_,Option<String>>(5)?,r.get::<_,Option<String>>(6)?,crate::diagnostics::response::column(r,7)?)))?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    rows.into_iter()
        .map(
            |(id, route, model, state, started_at, finished_at, error, mut diagnostics)| {
                if let Some(execution) = receipt(&id)? {
                    diagnostics.get_or_insert_with(|| serde_json::json!({}))["sourceExecution"] =
                        execution;
                }
                Ok(TranscriptionAttempt {
                    diagnostics,
                    id,
                    route: ConnectionRoute::parse(&route)?,
                    model,
                    state,
                    started_at,
                    finished_at,
                    error,
                })
            },
        )
        .collect()
}

/// A submitted message can combine several takes. Validate identities without
/// pretending an edited message is identical to a recognizer's transcript.
pub(crate) fn validate_sources(db: &Connection, conversation: &str, ids: &[String]) -> Result<()> {
    let unique: std::collections::BTreeSet<_> = ids.iter().collect();
    if ids.len() > 128 || unique.len() != ids.len() {
        return Err(AppError::new(
            ErrorCode::Validation,
            "Invalid recording sources.",
        ));
    }
    for id in ids {
        let valid:bool=db.query_row("SELECT EXISTS(SELECT 1 FROM transcription_attempts WHERE id=?1 AND conversation_id=?2 AND state='succeeded')",params![id,conversation],|r|r.get(0))?;
        if !valid {
            return Err(AppError::new(
                ErrorCode::Validation,
                "Recording source does not belong to this conversation.",
            ));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::store::Store;
    fn setup() -> (tempfile::TempDir, Store, RecordingOwner, ResolvedTarget) {
        let dir = tempfile::tempdir().unwrap();
        let mut store = Store::open(&dir.path().join("audio.sqlite3")).unwrap();
        store.prepare_chat().unwrap();
        store
            .connection
            .execute("UPDATE ai_config SET route='hosted'", [])
            .unwrap();
        store
            .set_hosted_connection(1, Some("chat-reference"), "fixture@example.invalid")
            .unwrap();
        store
            .connection
            .execute(
                "UPDATE ai_config SET hosted_credential_id='audio-reference'",
                [],
            )
            .unwrap();
        let owner =
            RecordingOwner::Conversation(store.snapshot().unwrap().conversations[0].id.clone());
        let target = crate::ai::connections::access::resolve(
            &store.connection,
            crate::ai::connections::access::Capability::Transcription,
        )
        .unwrap();
        (dir, store, owner, target)
    }
    #[test]
    fn receipt_is_single_use_and_private_text_is_not_retained() {
        let (_dir, mut store, owner, target) = setup();
        store
            .begin_transcription("recording", &owner, &target)
            .unwrap();
        assert!(
            store
                .begin_transcription("recording", &owner, &target)
                .is_err()
        );
        assert_eq!(
            store
                .finish_transcription("recording", &owner, &target, Ok("Private speech".into()))
                .unwrap(),
            "Private speech"
        );
        assert!(
            store
                .finish_transcription("recording", &owner, &target, Ok("Duplicate".into()))
                .is_err()
        );
        let records = views(&store.connection, &owner).unwrap();
        assert_eq!(records.len(), 1);
        assert_eq!(records[0].state, "succeeded");
        assert!(
            !serde_json::to_string(&records)
                .unwrap()
                .contains("Private speech")
        );
        assert!(
            store
                .conversation_snapshot(owner.id(), None)
                .unwrap()
                .messages
                .is_empty()
        );
        let stats = store.profile().unwrap();
        assert_eq!(stats.global.attempts, 0);
        assert_eq!(stats.global.unknown_usage, 0);
        assert_eq!(stats.global.input_tokens, 0);
        let revision = store.snapshot().unwrap().conversations[0].revision;
        let conversation = owner.id().to_owned();
        store
            .execute(Command {
                session_id: store.session_id.clone(),
                action_id: uuid::Uuid::new_v4().to_string(),
                action: Action::DeleteConversation {
                    conversation_id: conversation,
                    expected_revision: revision,
                },
            })
            .unwrap();
        assert_eq!(store.profile().unwrap().global.attempts, 0);
    }
    #[test]
    fn archived_owner_or_deleted_source_cannot_publish_or_recreate_receipts() {
        let (_dir, mut store, owner, target) = setup();
        store
            .begin_transcription("archived", &owner, &target)
            .unwrap();
        store
            .connection
            .execute("UPDATE contacts SET archived=1", [])
            .unwrap();
        assert!(
            store
                .finish_transcription("archived", &owner, &target, Ok("Late speech".into()))
                .is_err()
        );
        assert_eq!(
            views(&store.connection, &owner).unwrap()[0].state,
            "unknown"
        );
        store
            .connection
            .execute("UPDATE contacts SET archived=0", [])
            .unwrap();
        store
            .begin_transcription("deleted", &owner, &target)
            .unwrap();
        let revision = store.snapshot().unwrap().conversations[0].revision;
        let conversation = owner.id().to_owned();
        store
            .execute(Command {
                session_id: store.session_id.clone(),
                action_id: uuid::Uuid::new_v4().to_string(),
                action: Action::DeleteConversation {
                    conversation_id: conversation.clone(),
                    expected_revision: revision,
                },
            })
            .unwrap();
        assert!(
            store
                .finish_transcription("deleted", &owner, &target, Ok("Late speech".into()))
                .is_err()
        );
        assert!(views(&store.connection, &owner).unwrap().is_empty());
    }

    #[test]
    fn interrupted_audio_stays_unknown_across_restarts_without_replay() {
        let (dir, mut store, owner, target) = setup();
        store
            .begin_transcription("recording", &owner, &target)
            .unwrap();
        drop(store);
        for _ in 0..2 {
            let store = Store::open(&dir.path().join("audio.sqlite3")).unwrap();
            let records = views(&store.connection, &owner).unwrap();
            assert_eq!(records.len(), 1);
            assert_eq!(records[0].state, "unknown");
            assert!(records[0].finished_at.is_some());
        }
    }
    #[test]
    fn changed_target_defeats_publication_and_pause_prevents_dispatch() {
        let (_dir, mut store, owner, target) = setup();
        store
            .connection
            .execute("UPDATE ai_config SET paused=1", [])
            .unwrap();
        assert!(
            store
                .begin_transcription("paused", &owner, &target)
                .is_err()
        );
        assert!(views(&store.connection, &owner).unwrap().is_empty());
        store
            .connection
            .execute("UPDATE ai_config SET paused=0", [])
            .unwrap();
        store
            .begin_transcription("recording", &owner, &target)
            .unwrap();
        store
            .connection
            .execute(
                "UPDATE ai_config SET revision=revision+1,hosted_credential_id=NULL",
                [],
            )
            .unwrap();
        assert_eq!(
            store
                .finish_transcription("recording", &owner, &target, Ok("Late speech".into()))
                .unwrap_err()
                .code,
            ErrorCode::UnknownOutcome
        );
        assert_eq!(
            views(&store.connection, &owner).unwrap()[0].state,
            "unknown"
        );
    }
    #[test]
    fn unrelated_settings_revision_does_not_discard_recorded_words() {
        let (_dir, mut store, owner, target) = setup();
        store
            .connection
            .execute(
                "UPDATE ai_config SET revision=revision+1,standard_model='other-chat-model'",
                [],
            )
            .unwrap();
        store
            .begin_transcription("recording", &owner, &target)
            .unwrap();
        store
            .connection
            .execute("UPDATE ai_config SET revision=revision+1", [])
            .unwrap();
        assert_eq!(
            store
                .finish_transcription("recording", &owner, &target, Ok("Recorded words".into()))
                .unwrap(),
            "Recorded words"
        );
    }
}
