use super::*;

#[test]
fn rollback_lost_acknowledgment_and_same_revision_stale_writers_are_safe() {
    let dir = tempfile::tempdir().unwrap();
    let mut store = SqlStore::open(&dir.path().join("owner.db"));
    let graph = Arc::new(registry().compile(definition()).unwrap());
    let mut host = DurableEngine::create([graph.clone()], limits(), &mut store).unwrap();
    start(&mut host, &mut store, &graph);
    host.apply(Event::Recover, &mut store).unwrap();
    let mut stale =
        DurableEngine::recover(checkpoint(&store), [graph.clone()], limits(), &mut store).unwrap();
    let before = store.bytes();
    let stamp = host.stamp().clone();
    store.fail_before_commit = true;
    assert_eq!(
        host.compact(&mut store).unwrap_err().code,
        "injected_rollback"
    );
    assert_eq!(store.bytes(), before);
    assert_eq!(host.stamp(), &stamp);
    assert_eq!(archives(&store), 0);
    assert!(!host.poisoned());
    store.fail_before_commit = false;
    store.fail_after_commit = true;
    assert_eq!(
        host.compact(&mut store).unwrap_err().code,
        "acknowledgment_lost"
    );
    assert!(host.poisoned());
    assert_eq!(
        host.compact(&mut store).unwrap_err().code,
        "reload_required"
    );
    assert_eq!(archives(&store), 1);
    store.fail_after_commit = false;
    assert_eq!(
        stale.compact(&mut store).unwrap_err().code,
        "stale_checkpoint"
    );
    let mut recovered =
        DurableEngine::recover(checkpoint(&store), [graph], limits(), &mut store).unwrap();
    assert_eq!(recovered.stamp().revision, stamp.revision);
    assert_ne!(recovered.stamp().checksum, stamp.checksum);
    assert!(!recovered.compact(&mut store).unwrap());
    assert_eq!(archives(&store), 1);
    assert!(store.publications().is_empty());
}

#[test]
fn history_limits_refuse_compaction_without_losing_the_current_journal() {
    for history in [
        HistoryLimits {
            segments: 0,
            ..history_limits()
        },
        HistoryLimits {
            events: 0,
            ..history_limits()
        },
        HistoryLimits {
            bytes: 0,
            ..history_limits()
        },
    ] {
        let dir = tempfile::tempdir().unwrap();
        let mut store = SqlStore::open(&dir.path().join("owner.db"));
        let graph = Arc::new(registry().compile(definition()).unwrap());
        let mut host = DurableEngine::create(
            [graph.clone()],
            DurableLimits {
                history,
                ..limits()
            },
            &mut store,
        )
        .unwrap();
        start(&mut host, &mut store, &graph);
        let before = store.bytes();
        assert_eq!(host.compact(&mut store).unwrap_err().code, "history_limit");
        assert_eq!(store.bytes(), before);
        assert_eq!(archives(&store), 0);
        assert!(!host.poisoned());
    }
}

#[test]
fn a_larger_snapshot_cannot_consume_reserved_capacity_or_replace_the_old_record() {
    let dir = tempfile::tempdir().unwrap();
    let mut store = SqlStore::open(&dir.path().join("owner.db"));
    let graph = Arc::new(registry().compile(definition()).unwrap());
    let mut reference = Engine::new([graph.clone()]).unwrap();
    begin(&mut reference, &graph, "run", 40);
    let reference = Checkpoint::capture(
        &reference,
        &uuid::Uuid::new_v4().to_string(),
        limits().checkpoint,
    )
    .unwrap();
    let bounds = DurableLimits {
        checkpoint: CheckpointLimits {
            bytes: reference.bytes().len() + 64,
            events: 100,
        },
        ..limits()
    };
    let mut host = DurableEngine::create([graph.clone()], bounds, &mut store).unwrap();
    start(&mut host, &mut store, &graph);
    let before = store.bytes();
    assert_eq!(
        host.compact(&mut store).unwrap_err().code,
        "checkpoint_byte_limit"
    );
    assert_eq!(store.bytes(), before);
    assert_eq!(archives(&store), 0);
    assert!(!host.poisoned());
}

#[test]
fn recovery_after_compacted_dispatch_is_unknown_and_cannot_reissue_work() {
    let dir = tempfile::tempdir().unwrap();
    let mut store = SqlStore::open(&dir.path().join("owner.db"));
    let graph = Arc::new(registry().compile(definition()).unwrap());
    let mut host = DurableEngine::create([graph.clone()], limits(), &mut store).unwrap();
    start(&mut host, &mut store, &graph);
    host.apply(capacity(1), &mut store).unwrap();
    let first = id(&host, "first");
    let _invocation = host.claim("run", "first", first, &mut store).unwrap();
    host.compact(&mut store).unwrap();
    let before = host.stamp().revision;
    let mut host =
        DurableEngine::recover(checkpoint(&store), [graph.clone()], limits(), &mut store).unwrap();
    assert_eq!(host.stamp().revision, before + 1);
    assert_eq!(
        host.inspect("run").unwrap().nodes["first"],
        Disposition::Unknown
    );
    assert!(host.claim("run", "first", first, &mut store).is_err());
    let bytes = store.bytes();
    DurableEngine::recover(checkpoint(&store), [graph], limits(), &mut store).unwrap();
    assert_eq!(store.bytes(), bytes);
    assert!(store.publications().is_empty());
}

#[test]
fn snapshot_suffix_saturation_keeps_settlement_adoption_and_recovery_space() {
    for by_bytes in [false, true] {
        for failed in [false, true] {
            let dir = tempfile::tempdir().unwrap();
            let mut store = SqlStore::open(&dir.path().join("owner.db"));
            let graph = Arc::new(registry().compile(definition()).unwrap());
            let limits = DurableLimits {
                checkpoint: if by_bytes {
                    CheckpointLimits {
                        bytes: 8_000,
                        events: 1000,
                    }
                } else {
                    CheckpointLimits {
                        bytes: 100_000,
                        events: 6,
                    }
                },
                ..limits()
            };
            let mut host = DurableEngine::create([graph.clone()], limits, &mut store).unwrap();
            start(&mut host, &mut store, &graph);
            let work = host.apply(capacity(1), &mut store).unwrap().remove(0);
            let first = id(&host, "first");
            let _invocation = host.claim("run", "first", first, &mut store).unwrap();
            host.compact(&mut store).unwrap();
            let parent = checkpoint(&store).archive_parent().unwrap().clone();
            let original = store.read_archive(&parent, limits.history.bytes).unwrap();
            let mut exhausted = false;
            for _ in 0..1000 {
                if let Err(error) = host.apply(
                    Event::Pause {
                        run: "run".into(),
                        paused: false,
                    },
                    &mut store,
                ) {
                    assert_eq!(
                        error.code,
                        if by_bytes {
                            "checkpoint_byte_limit"
                        } else {
                            "checkpoint_event_limit"
                        }
                    );
                    exhausted = true;
                    break;
                }
            }
            assert!(exhausted);
            let mut event = Event::Settle {
                execution: work.execution,
                outcome: if failed {
                    Err(unclassified("fixture", ""))
                } else {
                    Ok(values(41))
                },
            };
            if failed {
                let overhead = serde_json::to_vec(&event).unwrap().len();
                if let Event::Settle {
                    outcome: Err(f), ..
                } = &mut event
                {
                    f.path = "x".repeat(limits.settlement_event_bytes - overhead);
                }
                assert_eq!(
                    serde_json::to_vec(&event).unwrap().len(),
                    limits.settlement_event_bytes
                );
            }
            host.apply(event, &mut store).unwrap();
            if !failed {
                host.adopt("run", "first", first, &mut store).unwrap();
            }
            let host =
                DurableEngine::recover(checkpoint(&store), [graph], limits, &mut store).unwrap();
            assert_eq!(
                host.inspect("run").unwrap().nodes["first"],
                if failed {
                    Disposition::Failed
                } else {
                    Disposition::Adopted
                }
            );
            assert_eq!(
                store.read_archive(&parent, limits.history.bytes).unwrap(),
                original
            );
            assert!(store.bytes().len() <= limits.checkpoint.bytes);
        }
    }
}
