use super::*;
#[test]
fn capture_accepts_overlapping_identical_requests() {
    let mut capture = State::new(
        Path::new("fixture"),
        Mode::Capture,
        Tape {
            format: 1,
            records: vec![],
        },
    )
    .unwrap();
    capture.start().unwrap();
    let first = capture.begin(Kind::Protocol, "a".repeat(64)).unwrap().2;
    let second = capture.begin(Kind::Protocol, "a".repeat(64)).unwrap().2;
    assert_ne!(first, second);
    assert_eq!(capture.active.len(), 2);
    // Completion order differs from admission order. Each response stays with its call.
    capture
        .push(second, serde_json::json!({"Ok": "second"}))
        .unwrap();
    capture.finish(second).unwrap();
    capture
        .push(first, serde_json::json!({"Ok": "first"}))
        .unwrap();
    capture.finish(first).unwrap();
    let recorded = capture.stop().unwrap().unwrap();
    assert_eq!(recorded.records[0].frames[0].payload["Ok"], "first");
    assert_eq!(recorded.records[1].frames[0].payload["Ok"], "second");
    let mut replay = State::new(Path::new("fixture"), Mode::Replay, recorded).unwrap();
    replay.start().unwrap();
    let (_, first_record, first) = replay.begin(Kind::Protocol, "a".repeat(64)).unwrap();
    let (_, second_record, second) = replay.begin(Kind::Protocol, "a".repeat(64)).unwrap();
    assert_eq!(first_record.unwrap().frames[0].payload["Ok"], "first");
    assert_eq!(second_record.unwrap().frames[0].payload["Ok"], "second");
    replay.finish(second).unwrap();
    replay.finish(first).unwrap();
    replay.stop().unwrap();
}
#[test]
fn cancelling_one_identical_call_preserves_the_other() {
    let mut capture = State::new(
        Path::new("fixture"),
        Mode::Capture,
        Tape {
            format: 1,
            records: vec![],
        },
    )
    .unwrap();
    capture.start().unwrap();
    let first = capture.begin(Kind::Text, "a".repeat(64)).unwrap().2;
    let second = capture.begin(Kind::Text, "a".repeat(64)).unwrap().2;
    capture.cancel(first, true, false);
    capture
        .push(second, serde_json::json!({"Ok": true}))
        .unwrap();
    capture.finish(second).unwrap();
    let tape = capture.stop().unwrap().unwrap();
    assert!(tape.records[0].cancelled_micros.is_some());
    assert!(tape.records[1].cancelled_micros.is_none());
    assert!(tape.records.iter().all(|record| record.complete));
}
#[test]
fn failure_report_preserves_first_cause_without_content() {
    let directory = tempfile::tempdir().unwrap();
    let mut capture = State::new(
        directory.path(),
        Mode::Capture,
        Tape {
            format: 1,
            records: vec![],
        },
    )
    .unwrap();
    capture.start().unwrap();
    let id = capture.begin(Kind::Text, "a".repeat(64)).unwrap().2;
    capture.bytes = LIMIT;
    assert!(
        capture
            .push(id, serde_json::json!("sensitive response"))
            .is_err()
    );
    capture.fail("microphone_capture_invalidated");
    assert!(
        capture
            .stop()
            .err()
            .unwrap()
            .message
            .contains("capture_byte_limit")
    );
    let report =
        std::fs::read_to_string(directory.path().join("native-capture-failure.json")).unwrap();
    assert!(report.contains("capture_byte_limit"));
    assert!(!report.contains("sensitive response"));
    assert!(!report.contains(&"a".repeat(64)));
}
#[test]
fn live_network_is_rejected_and_permanently_invalidates_replay() {
    for mode in [Mode::Capture, Mode::Replay] {
        let mut state = State::new(
            Path::new("fixture"),
            mode,
            Tape {
                format: 1,
                records: vec![],
            },
        )
        .unwrap();
        state.start().unwrap();
        assert_eq!(state.require_live_network().is_err(), mode == Mode::Replay);
        assert_eq!(state.stop().is_err(), mode == Mode::Replay);
    }
}
fn tape(fingerprint: &str) -> Tape {
    Tape {
        format: 1,
        records: vec![Record {
            kind: Kind::Protocol,
            fingerprint: fingerprint.into(),
            start_micros: 0,
            frames: vec![Frame {
                elapsed_micros: 1,
                payload: serde_json::json!({"Ok":true}),
            }],
            complete: true,
            cancelled_micros: None,
        }],
    }
}
#[test]
fn armed_work_blocks_start_and_active_capture_cannot_finish() {
    let fingerprint = "a".repeat(64);
    let mut capture = State::new(
        Path::new("fixture"),
        Mode::Capture,
        Tape {
            format: 1,
            records: vec![],
        },
    )
    .unwrap();
    assert!(
        !capture
            .begin(Kind::Protocol, fingerprint.clone())
            .unwrap()
            .0
    );
    assert!(capture.start().is_err());
    capture.active.clear();
    capture.start().unwrap();
    assert!(capture.begin(Kind::Protocol, fingerprint).unwrap().0);
    assert!(capture.records[0].start_micros <= capture.epoch.elapsed().as_micros() as u64);
    assert!(capture.stop().is_err());
    assert!(capture.failed);
}
#[test]
fn replay_mismatch_and_repeated_request_permanently_fail() {
    let fingerprint = "a".repeat(64);
    let mut replay = State::new(Path::new("fixture"), Mode::Replay, tape(&fingerprint)).unwrap();
    replay.start().unwrap();
    assert!(replay.begin(Kind::Protocol, "b".repeat(64)).is_err());
    assert!(replay.begin(Kind::Protocol, fingerprint.clone()).is_err());
    assert!(replay.stop().is_err());
    let mut replay = State::new(Path::new("fixture"), Mode::Replay, tape(&fingerprint)).unwrap();
    replay.start().unwrap();
    assert!(
        replay
            .begin(Kind::Protocol, fingerprint.clone())
            .unwrap()
            .1
            .is_some()
    );
    replay.active.clear();
    assert!(replay.begin(Kind::Protocol, fingerprint).is_err());
    assert!(replay.failed);
}
#[test]
fn independent_fingerprints_overlap_but_ambiguous_requests_fail() {
    let a = "a".repeat(64);
    let b = "b".repeat(64);
    let mut source = tape(&a);
    source.records.extend(tape(&b).records);
    let mut replay = State::new(Path::new("fixture"), Mode::Replay, source).unwrap();
    replay.start().unwrap();
    replay.begin(Kind::Protocol, a.clone()).unwrap();
    replay.begin(Kind::Protocol, b).unwrap();
    assert_eq!(replay.active.len(), 2);
    assert!(replay.begin(Kind::Protocol, a).is_err());
}
#[test]
fn incomplete_recordings_and_unconsumed_outcomes_are_rejected() {
    let fingerprint = "a".repeat(64);
    let mut invalid = tape(&fingerprint);
    invalid.records[0].complete = false;
    assert!(State::new(Path::new("fixture"), Mode::Replay, invalid).is_err());
    let mut replay = State::new(Path::new("fixture"), Mode::Replay, tape(&fingerprint)).unwrap();
    replay.start().unwrap();
    assert!(replay.stop().is_err());
}
#[test]
fn known_credential_is_removed_recursively_without_dropping_content() {
    let value = without_secret(
        serde_json::json!({"text":"你好 secret مرحبا","metadata":{"reason":"secret"},"usage":3}),
        "secret",
    );
    assert_eq!(value["text"], "你好 [credential] مرحبا");
    assert_eq!(value["usage"], 3);
    assert!(!value.to_string().contains("secret"));
}
