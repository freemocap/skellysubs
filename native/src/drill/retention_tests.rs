use super::*;
use crate::drill::{DrillItemInput, stage_attempt};
fn setup() -> (tempfile::TempDir, Store, String) {
    let dir = tempfile::tempdir().unwrap();
    let mut store = Store::open(&dir.path().join("retention.sqlite3")).unwrap();
    let item = store
        .create_drill_item(DrillItemInput {
            text: "Hola".into(),
            language: "spanish".into(),
            variety: None,
            explanation: "english".into(),
            explanation_variety: None,
        })
        .unwrap();
    (dir, store, item.id)
}
#[test]
fn oldest_audio_is_pruned_across_pending_and_file_storage_without_losing_attempts() {
    let (_dir, mut store, item) = setup();
    let first = store
        .save_drill_attempt(&item, None, "hola", Some(vec![1, 2, 3]))
        .unwrap();
    let second = stage_attempt(&store.connection, &item, None, "ola", Some(&[4, 5, 6])).unwrap();
    let third = stage_attempt(&store.connection, &item, None, "hola", Some(&[7, 8, 9])).unwrap();
    assert_eq!(store.drill_storage().unwrap().recording_bytes, 9);
    let loaded = store.drill_attempt_audio(&first.id).unwrap();
    let tx = store.connection.transaction().unwrap();
    mark_to(&tx, 3).unwrap();
    tx.commit().unwrap();
    store.prune_drill_audio().unwrap();
    assert_eq!(loaded, [1, 2, 3]); // In-progress replay owns bytes, not a disk handle.
    assert!(store.drill_attempt_audio(&first.id).is_err());
    assert!(store.drill_attempt_audio(&second).is_err());
    assert_eq!(store.drill_attempt_audio(&third).unwrap(), [7, 8, 9]);
    assert_eq!(store.drill_storage().unwrap().recording_bytes, 3);
    let attempts = store.drill_items("spanish").unwrap().remove(0).attempts;
    assert_eq!(attempts.len(), 3);
    assert!(
        attempts.iter().all(|a| !a.transcript.is_empty()
            && a.comparison["policy"] == crate::drill::comparison::POLICY)
    );
}
#[test]
fn keep_none_is_durable_and_does_not_stage_future_recording_bytes() {
    let (dir, mut store, item) = setup();
    assert_eq!(store.drill_storage().unwrap().limit_mb, 500);
    // Any whole number of megabytes in range is a valid cap; only nonsense is refused.
    store.set_drill_storage(42).unwrap();
    assert_eq!(store.drill_storage().unwrap().limit_mb, 42);
    assert!(store.set_drill_storage(-1).is_err());
    assert!(store.set_drill_storage(MAX_LIMIT_MB + 1).is_err());
    assert_eq!(store.drill_storage().unwrap().limit_mb, 42);
    store.set_drill_storage(500).unwrap();
    let old = store
        .save_drill_attempt(&item, None, "hola", Some(vec![1, 2]))
        .unwrap();
    let path = store.drill_audio.join(format!("{}.wav.part", old.id));
    std::fs::write(&path, b"interrupted write").unwrap();
    store.set_drill_storage(0).unwrap();
    assert!(!path.exists());
    let new = stage_attempt(&store.connection, &item, None, "hola", Some(&[3, 4])).unwrap();
    let attempt = crate::drill::attempt(&store.connection, &new).unwrap();
    assert!(attempt.audio_bytes.is_none());
    assert!(attempt.audio_pruned_at.is_none()); // Never kept, not falsely called pruned.
    assert_eq!(store.drill_storage().unwrap().recording_bytes, 0);
    drop(store);
    let store = Store::open(&dir.path().join("retention.sqlite3")).unwrap();
    assert_eq!(store.drill_storage().unwrap().limit_mb, 0);
    assert_eq!(store.drill_items("spanish").unwrap()[0].attempts.len(), 2);
}
#[test]
fn cleanup_failure_is_retryable_and_restart_finishes_committed_pruning() {
    let (dir, mut store, item) = setup();
    let old = store
        .save_drill_attempt(&item, None, "hola", Some(vec![1, 2]))
        .unwrap();
    let path = store.drill_audio.join(format!("{}.wav", old.id));
    std::fs::remove_file(&path).unwrap();
    std::fs::create_dir(&path).unwrap(); // Deterministic IO failure, including root test users.
    let error = store.set_drill_storage(0).unwrap_err();
    assert_eq!(error.code, ErrorCode::Storage);
    assert!(error.diagnostics.is_some());
    assert_eq!(store.drill_storage().unwrap().pending_removal_bytes, 2);
    assert!(store.drill_attempt_audio(&old.id).is_err());
    std::fs::remove_dir(&path).unwrap();
    std::fs::write(&path, [1, 2]).unwrap();
    drop(store);
    let store = Store::open(&dir.path().join("retention.sqlite3")).unwrap();
    assert!(!path.exists());
    assert_eq!(store.drill_storage().unwrap().recording_bytes, 0);
    assert_eq!(store.drill_storage().unwrap().pending_removal_bytes, 0);
    assert_eq!(store.drill_items("spanish").unwrap()[0].attempts.len(), 1);
}

#[test]
fn pruning_pending_blobs_reclaims_database_pages_not_just_logical_audio_bytes() {
    let (_dir, mut store, item) = setup();
    stage_attempt(
        &store.connection,
        &item,
        None,
        "hola",
        Some(&vec![1; 1_000_000]),
    )
    .unwrap();
    let before: i64 = store
        .connection
        .pragma_query_value(None, "page_count", |r| r.get(0))
        .unwrap();
    store.set_drill_storage(0).unwrap();
    let after: i64 = store
        .connection
        .pragma_query_value(None, "page_count", |r| r.get(0))
        .unwrap();
    assert!(
        after < before - 100,
        "pending media pages must return to the filesystem: {before} -> {after}"
    );
}

#[test]
fn publication_under_keep_none_keeps_receipt_and_comparison_but_no_audio() {
    let (_dir, mut store, item) = setup();
    store.connection.execute("UPDATE ai_config SET route='custom',custom_config=json_set(custom_config,'$.baseUrl','http://127.0.0.1:8765/v1','$.bearerAuth',json('false'))", []).unwrap();
    let target = crate::ai::connections::access::resolve(
        &store.connection,
        crate::ai::connections::access::Capability::Transcription,
    )
    .unwrap();
    let owner = crate::drill::owner(&item);
    store
        .begin_transcription("recording", &owner, &target)
        .unwrap();
    store.set_drill_storage(0).unwrap(); // Change while the provider is in flight.
    store
        .publish_transcription(
            "recording",
            &owner,
            &target,
            Ok("Hola".into()),
            None,
            Some(b"recorded audio"),
        )
        .unwrap();
    let items = store.drill_items("spanish").unwrap();
    assert_eq!(items[0].attempts.len(), 1);
    assert_eq!(items[0].attempts[0].transcript, "Hola");
    assert!(items[0].attempts[0].audio_bytes.is_none());
    let receipts =
        crate::speech::recording::transcription::views(&store.connection, &owner).unwrap();
    assert_eq!(receipts[0].state, "succeeded");
    assert_eq!(store.profile().unwrap().global.attempts, 0);
    assert_eq!(store.drill_storage().unwrap().recording_bytes, 0);
    assert!(!store.drill_audio.exists());
}
