use super::*;
use crate::speech::recording::owner::RecordingOwner;

fn setup() -> (tempfile::TempDir, Store) {
    let dir = tempfile::tempdir().unwrap();
    let store = Store::open(&dir.path().join("drill.sqlite3")).unwrap();
    (dir, store)
}

fn phrase(text: &str) -> DrillItemInput {
    DrillItemInput {
        text: text.into(),
        language: "spanish".into(),
        variety: None,
        explanation: "english".into(),
        explanation_variety: None,
    }
}

#[test]
fn a_typed_phrase_becomes_an_item_a_recording_can_belong_to() {
    let (_dir, mut store) = setup();
    let item = store
        .create_drill_item(phrase("  Quisiera un café.  "))
        .unwrap();
    // Surrounding space is not part of what the learner practises.
    assert_eq!(item.text, "Quisiera un café.");
    assert_eq!(item.language, "spanish");
    assert!(!item.variety.is_empty() && !item.explanation_variety.is_empty());
    assert!(item.attempts.is_empty());
    // The item resolves its own recording scope, so capture needs nothing else.
    let scope = owner(&item.id).scope(&store).unwrap();
    assert_eq!(scope.language.language_id, "spanish");
    assert_eq!(scope.context.as_deref(), Some("Quisiera un café."));
    assert_eq!(store.drill_items("spanish").unwrap().len(), 1);
    assert!(store.drill_items("arabic").unwrap().is_empty());
    // A phrase with nothing in it, or an unknown language, is refused.
    assert!(store.create_drill_item(phrase("   ")).is_err());
    assert!(store.create_drill_item(phrase(&"x".repeat(513))).is_err());
    let mut unknown = phrase("Hola");
    unknown.language = "klingon".into();
    assert!(store.create_drill_item(unknown).is_err());
}

#[test]
fn an_attempt_keeps_its_transcript_comparison_and_audio_for_replay() {
    let (_dir, mut store) = setup();
    let item = store
        .create_drill_item(phrase("Quisiera un café."))
        .unwrap();
    let wav = b"RIFF....WAVEfixture".to_vec();
    let first = store
        .save_drill_attempt(&item.id, None, "quisiera un cafe", Some(wav.clone()))
        .unwrap();
    assert_eq!(first.sequence, 1);
    assert_eq!(first.transcript, "quisiera un cafe");
    assert_eq!(first.audio_bytes, Some(wav.len() as i64));
    assert!(first.audio_pruned_at.is_none());
    assert_eq!(first.comparison["policy"], comparison::POLICY);
    // Stored comparisons use the shared mark-insensitive matching policy.
    assert_eq!(first.comparison["edits"], 0);
    assert_eq!(first.comparison["matchRatio"], 1.0);
    assert_eq!(first.comparison["target"], "Quisiera un café.");
    assert_eq!(store.drill_attempt_audio(&first.id).unwrap(), wav);

    // A second attempt follows the first, and the newest is listed first.
    let second = store
        .save_drill_attempt(&item.id, None, "Quisiera un café.", None)
        .unwrap();
    assert_eq!(second.sequence, 2);
    assert_eq!(second.comparison["edits"], 0);
    assert_eq!(second.audio_bytes, None);
    let listed = store.drill_items("spanish").unwrap();
    assert_eq!(
        listed[0]
            .attempts
            .iter()
            .map(|attempt| attempt.sequence)
            .collect::<Vec<_>>(),
        vec![2, 1]
    );
    // An attempt whose audio was never kept says so rather than failing oddly.
    assert_eq!(
        store.drill_attempt_audio(&second.id).unwrap_err().code,
        ErrorCode::NotFound
    );
    // An item that is gone cannot take new attempts.
    store.delete_drill_item(&item.id).unwrap();
    assert!(
        store
            .save_drill_attempt(&item.id, None, "late", None)
            .is_err()
    );
}

#[test]
fn attempts_survive_leaving_and_returning_and_deletion_removes_everything() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("drill.sqlite3");
    let (item_id, attempt_id, audio_path) = {
        let mut store = Store::open(&path).unwrap();
        let item = store.create_drill_item(phrase("Hasta luego.")).unwrap();
        let attempt = store
            .save_drill_attempt(&item.id, None, "hasta luego", Some(b"RIFFfixture".to_vec()))
            .unwrap();
        (
            item.id,
            attempt.id.clone(),
            store.drill_audio.join(format!("{}.wav", attempt.id)),
        )
    };
    assert!(audio_path.exists());
    {
        // Reopening the workspace is what leaving and re-entering Drill does.
        let mut store = Store::open(&path).unwrap();
        let items = store.drill_items("spanish").unwrap();
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].attempts.len(), 1);
        assert_eq!(items[0].attempts[0].id, attempt_id);
        assert_eq!(items[0].attempts[0].transcript, "hasta luego");
        assert!(store.drill_attempt_audio(&attempt_id).is_ok());

        store.delete_drill_item(&item_id).unwrap();
        assert!(store.drill_items("spanish").unwrap().is_empty());
        assert!(store.drill_attempt_audio(&attempt_id).is_err());
        // Deleting the item takes its audio off the disk too.
        assert!(!audio_path.exists());
        assert!(store.delete_drill_item(&item_id).is_err());
    }
    let store = Store::open(&path).unwrap();
    assert!(store.drill_items("spanish").unwrap().is_empty());
}

#[test]
fn a_drill_recording_receipt_belongs_to_its_item_and_goes_with_it() {
    let (_dir, mut store) = setup();
    store
        .connection
        .execute(
            "UPDATE ai_config SET route='hosted',hosted_credential_id='audio-reference'",
            [],
        )
        .unwrap();
    store
        .set_hosted_connection(1, Some("chat-reference"), "fixture@example.invalid")
        .unwrap();
    let item = store.create_drill_item(phrase("Buenos días.")).unwrap();
    let target = crate::ai::connections::access::resolve(
        &store.connection,
        crate::ai::connections::access::Capability::Transcription,
    )
    .unwrap();
    let owner = RecordingOwner::DrillItem(item.id.clone());
    store
        .begin_transcription("recording", &owner, &target)
        .unwrap();
    store
        .finish_transcription("recording", &owner, &target, Ok("buenos dias".into()))
        .unwrap();
    let attempt = store
        .save_drill_attempt(&item.id, Some("recording"), "buenos dias", None)
        .unwrap();
    assert_eq!(
        attempt.transcription_attempt_id.as_deref(),
        Some("recording")
    );
    // Product publication alone does not record provider execution.
    let profile = store.profile().unwrap();
    assert_eq!(profile.global.attempts, 0);
    assert!(profile.personas.iter().all(|persona| persona.attempts == 0));
    // Deleting the item takes its recording receipt with it.
    store.delete_drill_item(&item.id).unwrap();
    assert!(
        crate::speech::recording::transcription::views(&store.connection, &owner)
            .unwrap()
            .is_empty()
    );
    assert_eq!(store.profile().unwrap().global.attempts, 0);
}

#[test]
fn an_attempt_is_stored_once_per_recording_and_only_for_its_own_item() {
    let (_dir, mut store) = setup();
    store
        .connection
        .execute(
            "UPDATE ai_config SET route='hosted',hosted_credential_id='audio-reference'",
            [],
        )
        .unwrap();
    store
        .set_hosted_connection(1, Some("chat-reference"), "fixture@example.invalid")
        .unwrap();
    let item = store.create_drill_item(phrase("Buenos días.")).unwrap();
    let other = store.create_drill_item(phrase("Hasta luego.")).unwrap();
    let target = crate::ai::connections::access::resolve(
        &store.connection,
        crate::ai::connections::access::Capability::Transcription,
    )
    .unwrap();
    let owner = RecordingOwner::DrillItem(item.id.clone());
    store
        .begin_transcription("recording", &owner, &target)
        .unwrap();

    // The receipt is still running: nothing produced this transcript yet.
    assert!(
        store
            .save_drill_attempt(&item.id, Some("recording"), "buenos dias", None)
            .is_err()
    );
    store
        .finish_transcription("recording", &owner, &target, Ok("buenos dias".into()))
        .unwrap();
    let first = store
        .save_drill_attempt(&item.id, Some("recording"), "buenos dias", None)
        .unwrap();
    // A retry after a failed save returns the attempt that already exists
    // rather than recording the same utterance twice.
    let again = store
        .save_drill_attempt(&item.id, Some("recording"), "buenos dias", None)
        .unwrap();
    assert_eq!(again.id, first.id);
    assert_eq!(store.drill_items("spanish").unwrap()[1].attempts.len(), 1);
    // Another item cannot claim this recording.
    assert!(
        store
            .save_drill_attempt(&other.id, Some("recording"), "buenos dias", None)
            .is_err()
    );
    // Nor can an unknown one.
    assert!(
        store
            .save_drill_attempt(&item.id, Some("missing"), "buenos dias", None)
            .is_err()
    );
    // A failed recording is not something the learner said.
    store
        .begin_transcription("failed", &owner, &target)
        .unwrap();
    store
        .finish_transcription(
            "failed",
            &owner,
            &target,
            Err(AppError::new(ErrorCode::Provider, "no transcript")),
        )
        .unwrap_err();
    assert!(
        store
            .save_drill_attempt(&item.id, Some("failed"), "buenos dias", None)
            .is_err()
    );
}

#[test]
fn audio_that_nothing_claims_is_cleaned_up_and_a_failed_delete_keeps_its_records() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("drill.sqlite3");
    let orphan;
    let kept;
    {
        let mut store = Store::open(&path).unwrap();
        let item = store.create_drill_item(phrase("Buenos días.")).unwrap();
        let saved = store
            .save_drill_attempt(&item.id, None, "buenos dias", Some(b"RIFFkept".to_vec()))
            .unwrap();
        kept = store.drill_audio.join(format!("{}.wav", saved.id));
        // What an interrupted save or a half-finished deletion leaves behind.
        orphan = store.drill_audio.join("abandoned.wav");
        std::fs::write(&orphan, b"RIFForphan").unwrap();
        assert_eq!(store.reconcile_drill_audio().unwrap(), 1);
        assert!(!orphan.exists());
        assert!(kept.exists());
    }
    // Startup reconciles it too, so a crash cannot leave audio behind forever.
    std::fs::write(&orphan, b"RIFForphan").unwrap();
    let store = Store::open(&path).unwrap();
    assert!(!orphan.exists());
    assert!(kept.exists());
    assert_eq!(store.drill_items("spanish").unwrap()[0].attempts.len(), 1);
}

#[test]
fn a_failed_audio_write_still_records_what_was_said() {
    let (_dir, mut store) = setup();
    let item = store.create_drill_item(phrase("Buenos días.")).unwrap();
    // The audio folder is taken by a file, so no recording can be written there.
    std::fs::write(&store.drill_audio, b"not a folder").unwrap();
    let error = store
        .save_drill_attempt(&item.id, None, "buenos dias", Some(b"RIFF".to_vec()))
        .unwrap_err();
    assert_eq!(error.code, ErrorCode::Storage);
    // The attempt itself survives: its transcript and score are not lost
    // because the audio could not be kept.
    let attempts = &store.drill_items("spanish").unwrap()[0].attempts;
    assert_eq!(attempts.len(), 1);
    assert_eq!(attempts[0].transcript, "buenos dias");
    assert_eq!(attempts[0].audio_bytes, Some(4));
    let id = attempts[0].id.clone();
    std::fs::remove_file(&store.drill_audio).unwrap();
    assert_eq!(store.drill_attempt_audio(&id).unwrap(), b"RIFF");
    assert_eq!(store.drill_items("spanish").unwrap()[0].attempts.len(), 1);
}

#[test]
fn native_publication_survives_page_loss_and_reopen_with_pending_audio() {
    let (dir, mut store) = setup();
    store
        .connection
        .execute(
            "UPDATE ai_config SET route='hosted',hosted_credential_id='audio-reference'",
            [],
        )
        .unwrap();
    store
        .set_hosted_connection(1, Some("chat-reference"), "fixture@example.invalid")
        .unwrap();
    let item = store.create_drill_item(phrase("Hola")).unwrap();
    let owner = RecordingOwner::DrillItem(item.id.clone());
    let target = crate::ai::connections::access::resolve(
        &store.connection,
        crate::ai::connections::access::Capability::Transcription,
    )
    .unwrap();
    store
        .begin_transcription("durable", &owner, &target)
        .unwrap();
    store
        .publish_transcription(
            "durable",
            &owner,
            &target,
            Ok("hola".into()),
            None,
            Some(b"RIFFdurable"),
        )
        .unwrap();
    let first = store.drill_items("spanish").unwrap()[0].attempts[0].clone();
    assert_eq!(first.transcription_attempt_id.as_deref(), Some("durable"));
    assert_eq!(first.audio_bytes, Some(11));
    drop(store);
    let store = Store::open(&dir.path().join("drill.sqlite3")).unwrap();
    assert_eq!(
        store.drill_attempt_audio(&first.id).unwrap(),
        b"RIFFdurable"
    );
    assert_eq!(store.drill_items("spanish").unwrap()[0].attempts.len(), 1);
}

#[test]
fn failed_attempt_publication_does_not_commit_a_successful_receipt() {
    let (_dir, mut store) = setup();
    store
        .connection
        .execute(
            "UPDATE ai_config SET route='hosted',hosted_credential_id='audio-reference'",
            [],
        )
        .unwrap();
    store
        .set_hosted_connection(1, Some("chat-reference"), "fixture@example.invalid")
        .unwrap();
    let item = store.create_drill_item(phrase("Hola")).unwrap();
    let owner = RecordingOwner::DrillItem(item.id.clone());
    let target = crate::ai::connections::access::resolve(
        &store.connection,
        crate::ai::connections::access::Capability::Transcription,
    )
    .unwrap();
    store
        .begin_transcription("atomic", &owner, &target)
        .unwrap();
    store.connection.execute_batch("CREATE TRIGGER reject_drill BEFORE INSERT ON drill_attempts BEGIN SELECT RAISE(ABORT,'fixture failure'); END;").unwrap();
    assert!(
        store
            .publish_transcription(
                "atomic",
                &owner,
                &target,
                Ok("hola".into()),
                None,
                Some(b"RIFF")
            )
            .is_err()
    );
    assert_eq!(
        crate::speech::recording::transcription::views(&store.connection, &owner).unwrap()[0].state,
        "running"
    );
    assert!(store.drill_items("spanish").unwrap()[0].attempts.is_empty());
}

#[test]
fn partial_deletion_keeps_recoverable_rows_and_marks_removed_audio() {
    let (_dir, mut store) = setup();
    let item = store.create_drill_item(phrase("Hola")).unwrap();
    let first = store
        .save_drill_attempt(&item.id, None, "hola", Some(b"one".to_vec()))
        .unwrap();
    let second = store
        .save_drill_attempt(&item.id, None, "hola", Some(b"two".to_vec()))
        .unwrap();
    let blocked = store.drill_audio.join(format!("{}.wav", second.id));
    std::fs::remove_file(&blocked).unwrap();
    std::fs::create_dir(&blocked).unwrap();
    assert!(store.delete_drill_item(&item.id).is_err());
    let remaining = store.drill_items("spanish").unwrap();
    assert_eq!(remaining.len(), 1);
    let removed = remaining[0]
        .attempts
        .iter()
        .find(|a| a.id == first.id)
        .unwrap();
    assert!(removed.audio_pruned_at.is_some());
    assert_eq!(removed.audio_bytes, None);
    std::fs::remove_dir(&blocked).unwrap();
    store.delete_drill_item(&item.id).unwrap();
    assert!(store.drill_items("spanish").unwrap().is_empty());
}

#[test]
fn takes_can_be_deleted_one_at_a_time_or_cleared_from_a_point_on() {
    let (_dir, mut store) = setup();
    let item = store.create_drill_item(phrase("Hasta luego.")).unwrap();
    let other = store.create_drill_item(phrase("Buenos días.")).unwrap();
    let wav = Some(b"RIFFfixture".to_vec());
    let first = store
        .save_drill_attempt(&item.id, None, "hasta luego", wav.clone())
        .unwrap();
    let second = store
        .save_drill_attempt(&item.id, None, "asta luego", wav.clone())
        .unwrap();
    let third = store
        .save_drill_attempt(&item.id, None, "hasta", wav.clone())
        .unwrap();
    let kept = store
        .save_drill_attempt(&other.id, None, "buenos días", None)
        .unwrap();
    let folder = store.drill_audio.clone();
    let audio = |id: &str| folder.join(format!("{id}.wav"));
    assert!(audio(&second.id).exists());

    store.delete_drill_attempt(&second.id).unwrap();
    assert!(!audio(&second.id).exists());
    assert_eq!(
        store.delete_drill_attempt(&second.id).unwrap_err().code,
        ErrorCode::NotFound
    );
    let ids = |store: &Store, id: &str| {
        store
            .drill_attempts(id, None, 20)
            .unwrap()
            .attempts
            .into_iter()
            .map(|a| a.id)
            .collect::<Vec<_>>()
    };
    assert_eq!(
        ids(&store, &item.id),
        vec![third.id.clone(), first.id.clone()]
    );

    // Only takes at or after the point are cleared; other phrases are untouched.
    store
        .connection
        .execute(
            "UPDATE drill_attempts SET created_at='2026-09-23T10:00:00.000Z' WHERE id=?1",
            [&first.id],
        )
        .unwrap();
    store
        .connection
        .execute(
            "UPDATE drill_attempts SET created_at='2026-09-23T11:00:00.000Z' WHERE id=?1",
            [&third.id],
        )
        .unwrap();
    assert!(
        store
            .clear_drill_attempts(&item.id, Some("yesterday"))
            .is_err()
    );
    assert_eq!(
        store
            .clear_drill_attempts(&item.id, Some("2026-09-23T10:30:00.000Z"))
            .unwrap(),
        1
    );
    assert_eq!(ids(&store, &item.id), vec![first.id.clone()]);
    assert_eq!(store.clear_drill_attempts(&item.id, None).unwrap(), 1);
    assert!(ids(&store, &item.id).is_empty());
    assert!(!audio(&first.id).exists());
    assert_eq!(ids(&store, &other.id), vec![kept.id]);
    assert_eq!(store.drill_items("spanish").unwrap().len(), 2);
    assert!(store.clear_drill_attempts("missing", None).is_err());
}

#[test]
fn unreliable_publication_preserves_text_but_never_improves_best_match() {
    let (_dir, mut store) = setup();
    store.connection.execute("UPDATE ai_config SET route='custom',custom_config=json_set(custom_config,'$.baseUrl','http://127.0.0.1:12345/v1','$.bearerAuth',json('false'))", []).unwrap();
    let item = store.create_drill_item(phrase("Hola")).unwrap();
    let owner = RecordingOwner::DrillItem(item.id.clone());
    let target = crate::ai::connections::access::resolve(
        &store.connection,
        crate::ai::connections::access::Capability::Transcription,
    )
    .unwrap();
    store
        .begin_transcription("uncertain", &owner, &target)
        .unwrap();
    let evidence = reliability::DrillReliability {
        policy: 1,
        accepted: false,
        confidence: Some(0.2),
        minimum_confidence: 0.6,
        speech_seconds: 0.8,
        no_speech_probability: None,
        source: "word_logprobs".into(),
        reason: "low_confidence".into(),
    };
    let diagnostics = serde_json::json!({"drill_reliability":evidence, "request_id":"receipt-1"});
    store
        .publish_transcription(
            "uncertain",
            &owner,
            &target,
            Ok("Hola".into()),
            Some(&diagnostics),
            Some(b"fixture wav"),
        )
        .unwrap();
    let saved = store.drill_items("spanish").unwrap().remove(0);
    assert_eq!(saved.best_match_ratio, None);
    let take = &saved.attempts[0];
    assert_eq!(take.transcript, "Hola");
    assert!(take.comparison["matchRatio"].is_null());
    assert_eq!(take.comparison["characterErrorRate"], 0.0);
    assert_eq!(take.comparison["reliability"]["confidence"], 0.2);
    let receipts =
        crate::speech::recording::transcription::views(&store.connection, &owner).unwrap();
    let retained = serde_json::to_string(&receipts).unwrap();
    assert!(retained.contains("receipt-1"));
    assert!(!retained.contains("Hola"));
}
