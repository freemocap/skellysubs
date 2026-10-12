use super::*;

#[test]
fn timeline_pages_are_native_snapshots_and_survive_compaction_without_mutations() {
    let dir = tempfile::tempdir().unwrap();
    let mut store = SqlStore::open(&dir.path().join("owner.db"));
    let mut engine = trace(&graph());
    let checkpoint = save(&engine, &mut store);
    let history = checkpoint
        .run_history(
            "a",
            None,
            100,
            read_limits(),
            export(100),
            &mut ReadOnly(&mut store),
        )
        .unwrap();
    assert!(history.frames.len() > 4);
    assert!(history.before.is_none());
    for frame in &history.frames {
        let expected = checkpoint
            .historical_inspection(frame.revision.parse().unwrap(), read_limits(), &mut store)
            .unwrap()
            .snapshot("a", export(100))
            .unwrap();
        assert_eq!(
            serde_json::to_value(frame).unwrap(),
            serde_json::to_value(expected).unwrap()
        );
        assert_eq!(frame.nodes.len(), 3);
        assert_eq!(frame.nodes["never"], Disposition::Disabled);
    }
    let expected = serde_json::to_value(&history.frames).unwrap();
    let compacted = compact(&mut engine, &checkpoint, &mut store);
    let original = store.bytes();
    let mut cursor = None;
    let mut frames = Vec::new();
    loop {
        let mut page = compacted
            .run_history(
                "a",
                cursor,
                2,
                read_limits(),
                export(100),
                &mut ReadOnly(&mut store),
            )
            .unwrap();
        assert!(page.frames.len() <= 2);
        cursor = page.before.as_ref().map(|value| value.parse().unwrap());
        page.frames.extend(frames);
        frames = page.frames;
        if cursor.is_none() {
            break;
        }
        assert!(frames.len() <= history.frames.len());
    }
    assert_eq!(serde_json::to_value(&frames).unwrap(), expected);
    let serialized = serde_json::to_string(&frames).unwrap();
    for secret in [
        "secret-code",
        "secret-content",
        "123456789",
        "test-authority-v1",
    ] {
        assert!(!serialized.contains(secret));
    }
    assert_eq!(store.bytes(), original);
    assert!(store.publications().is_empty());
}

#[test]
fn timeline_rejects_invalid_cuts_unknown_runs_and_insufficient_budgets() {
    let dir = tempfile::tempdir().unwrap();
    let mut store = SqlStore::open(&dir.path().join("owner.db"));
    let checkpoint = save(&trace(&graph()), &mut store);
    for (before, count) in [
        (Some(0), 2),
        (Some(checkpoint.stamp().revision + 1), 2),
        (None, 0),
    ] {
        assert_eq!(
            checkpoint
                .run_history("a", before, count, read_limits(), export(100), &mut store)
                .err()
                .unwrap()
                .code,
            "history_cursor"
        );
    }
    assert!(
        checkpoint
            .run_history("missing", None, 2, read_limits(), export(100), &mut store)
            .is_err()
    );
    assert_eq!(
        checkpoint
            .run_history(
                "a",
                None,
                2,
                read_limits(),
                ExportLimits {
                    bytes: 1,
                    attempts: 100
                },
                &mut store
            )
            .err()
            .unwrap()
            .code,
        "inspection_limit"
    );
}
