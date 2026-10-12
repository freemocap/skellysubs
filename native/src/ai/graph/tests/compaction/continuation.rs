use super::*;

fn trace(graph: &Arc<Executable>) -> Vec<Event> {
    let mut e = Engine::new([graph.clone()]).unwrap();
    begin(&mut e, graph, "a", 40);
    begin(&mut e, graph, "b", 40);
    let first = advance(&mut e, capacity(1)).remove(0);
    e.apply(Event::Cancel {
        run: "b".into(),
        node: None,
    })
    .unwrap();
    e.apply(Event::Settle {
        execution: first.execution,
        outcome: Ok(values(41)),
    })
    .unwrap();
    adopt(&mut e, "a", "first");
    e.apply(demand("a", "second")).unwrap();
    let second = advance(&mut e, capacity(1)).remove(0);
    e.apply(Event::Settle {
        execution: second.execution,
        outcome: Err(unclassified("fixture", "second")),
    })
    .unwrap();
    e.apply(Event::Retry {
        run: "a".into(),
        node: "second".into(),
    })
    .unwrap();
    advance(&mut e, capacity(1));
    e.apply(Event::Recover).unwrap();
    e.apply(Event::Pause {
        run: "a".into(),
        paused: false,
    })
    .unwrap();
    e.apply(Event::Retry {
        run: "a".into(),
        node: "second".into(),
    })
    .unwrap();
    let retry = advance(&mut e, capacity(1)).remove(0);
    e.apply(Event::Settle {
        execution: retry.execution,
        outcome: Ok(values(42)),
    })
    .unwrap();
    adopt(&mut e, "a", "second");
    e.journal().to_vec()
}

#[test]
fn every_cut_preserves_state_identity_and_all_remaining_transitions() {
    let mut d = definition();
    d.nodes.get_mut("second").unwrap().activation = Activation::OnDemand;
    let graph = Arc::new(registry().compile(d).unwrap());
    let events = trace(&graph);
    for cut in 1..=events.len() {
        let dir = tempfile::tempdir().unwrap();
        let mut store = SqlStore::open(&dir.path().join("owner.db"));
        let mut original = Engine::new([graph.clone()]).unwrap();
        for event in &events[..cut] {
            original.apply(event.clone()).unwrap();
        }
        let old = Checkpoint::capture(
            &original,
            &uuid::Uuid::new_v4().to_string(),
            limits().checkpoint,
        )
        .unwrap();
        store
            .commit(CommitRequest {
                expected: None,
                next: &old,
                intent: CommitIntent::Record,
                records: RecordChanges::between(None, &original.state),
            })
            .unwrap();
        let mut compacted = original.clone();
        compacted.rebase();
        let next = old.compacted(&compacted, limits().checkpoint).unwrap();
        assert_eq!(next.stamp().revision, old.stamp().revision);
        assert_ne!(next.stamp().checksum, old.stamp().checksum);
        assert_eq!(next.base_revision(), cut as u64);
        assert_eq!(next.suffix_len(), 0);
        store
            .commit(CommitRequest {
                expected: Some(old.stamp()),
                next: &next,
                intent: CommitIntent::Compact { archive: &old },
                records: RecordChanges::default(),
            })
            .unwrap();
        assert_eq!(
            store
                .read_archive(old.stamp(), limits().history.bytes)
                .unwrap(),
            old.bytes()
        );
        let decoded = checkpoint(&store);
        let (mut restored, _) = decoded
            .replay_history(
                [graph.clone()],
                limits().history,
                state_limits(),
                &mut store,
            )
            .unwrap();
        assert_eq!(restored.state.clone(), original.state.clone(), "cut {cut}");
        for run in ["a", "b"] {
            if original.state.runs.contains_key(run) {
                assert_eq!(
                    serde_json::to_value(restored.inspect(run).unwrap()).unwrap(),
                    serde_json::to_value(original.inspect(run).unwrap()).unwrap()
                );
            }
        }
        for event in &events[cut..] {
            let expected = original.apply(event.clone()).unwrap();
            assert_eq!(
                restored.apply(event.clone()).unwrap(),
                expected,
                "continuation at cut {cut}"
            );
            assert_eq!(restored.state.clone(), original.state.clone());
            assert_eq!(restored.revision(), original.revision());
        }
        assert_eq!(restored.outputs("a").unwrap(), Some(values(42)));
        assert!(store.publications().is_empty());
    }
}

#[tokio::test]
async fn an_inflight_invocation_and_later_demand_survive_compaction() {
    let dir = tempfile::tempdir().unwrap();
    let mut store = SqlStore::open(&dir.path().join("owner.db"));
    let mut d = definition();
    d.nodes.get_mut("second").unwrap().activation = Activation::OnDemand;
    let graph = Arc::new(registry().compile(d).unwrap());
    let mut host = DurableEngine::create([graph.clone()], limits(), &mut store).unwrap();
    start(&mut host, &mut store, &graph);
    let work = host.apply(capacity(1), &mut store).unwrap().remove(0);
    let first = id(&host, "first");
    let invocation = host.claim("run", "first", first, &mut store).unwrap();
    let before = view(&host);
    let stamp = host.stamp().clone();
    assert!(host.compact(&mut store).unwrap());
    assert_eq!(view(&host), before);
    assert_eq!(host.stamp().revision, stamp.revision);
    assert_ne!(host.stamp().checksum, stamp.checksum);
    assert!(!host.compact(&mut store).unwrap());
    assert!(host.claim("run", "first", first, &mut store).is_err());
    host.apply(
        Event::Settle {
            execution: work.execution,
            outcome: invocation.execute(evidence_limits()).await.outcome,
        },
        &mut store,
    )
    .unwrap();
    host.adopt("run", "first", first, &mut store).unwrap();
    host.compact(&mut store).unwrap();
    let mut host =
        DurableEngine::recover(checkpoint(&store), [graph], limits(), &mut store).unwrap();
    assert_eq!(
        host.inspect("run").unwrap().nodes["first"],
        Disposition::Adopted
    );
    host.apply(
        Event::Pause {
            run: "run".into(),
            paused: false,
        },
        &mut store,
    )
    .unwrap();
    host.apply(demand("run", "second"), &mut store).unwrap();
    let work = host.apply(capacity(1), &mut store).unwrap().remove(0);
    let second = id(&host, "second");
    assert!(second > first);
    let output = host
        .claim("run", "second", second, &mut store)
        .unwrap()
        .execute(evidence_limits())
        .await
        .outcome;
    host.apply(
        Event::Settle {
            execution: work.execution,
            outcome: output,
        },
        &mut store,
    )
    .unwrap();
    host.adopt("run", "second", second, &mut store).unwrap();
    assert_eq!(host.outputs("run").unwrap(), Some(values(42)));
    assert_eq!(store.publications().len(), 2);
    assert_eq!(archives(&store), 2);
}

#[test]
fn repeated_compaction_reclaims_suffix_events_without_resetting_revision() {
    let dir = tempfile::tempdir().unwrap();
    let mut store = SqlStore::open(&dir.path().join("owner.db"));
    let graph = Arc::new(registry().compile(definition()).unwrap());
    let limits = DurableLimits {
        checkpoint: CheckpointLimits {
            bytes: 1_000_000,
            events: 6,
        },
        ..limits()
    };
    let mut host = DurableEngine::create([graph.clone()], limits, &mut store).unwrap();
    start(&mut host, &mut store, &graph);
    for _ in 0..3 {
        for _ in checkpoint(&store).suffix_len()..5 {
            host.apply(
                Event::Pause {
                    run: "run".into(),
                    paused: false,
                },
                &mut store,
            )
            .unwrap();
        }
        assert!(
            host.apply(
                Event::Pause {
                    run: "run".into(),
                    paused: false,
                },
                &mut store,
            )
            .is_err()
        );
        let old = checkpoint(&store);
        assert_eq!(old.suffix_len(), 5);
        let before = view(&host);
        host.compact(&mut store).unwrap();
        assert_eq!(view(&host), before);
        assert_eq!(checkpoint(&store).suffix_len(), 0);
    }
    assert_eq!(host.stamp().revision, 15);
    let host = DurableEngine::recover(checkpoint(&store), [graph], limits, &mut store).unwrap();
    assert_eq!(host.stamp().revision, 16);
    assert_eq!(checkpoint(&store).suffix_len(), 1);
    assert_eq!(archives(&store), 3);
}

#[test]
fn logical_revision_overflow_rejects_without_mutation() {
    let mut engine = Engine::default();
    engine.journal_start = u64::MAX;
    assert_eq!(
        engine.apply(Event::Recover).unwrap_err().code,
        "revision_limit"
    );
    assert!(engine.journal().is_empty());
    assert_eq!(engine.revision(), u64::MAX);
}
