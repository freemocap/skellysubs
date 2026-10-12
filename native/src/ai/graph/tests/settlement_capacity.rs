use super::{durable_store::SqlStore, *};

fn limits(bytes: usize, events: usize) -> DurableLimits {
    DurableLimits {
        record_reads: record_read_limits(),
        checkpoint: CheckpointLimits { bytes, events },
        settlement_event_bytes: 256,
        history: history_limits(),
        state: state_limits(),
    }
}

fn graph(reuse: Reuse) -> Arc<Executable> {
    let mut registry = Registry::default();
    registry
        .define_type(contract("integer"), Shape::Integer)
        .unwrap();
    registry.define_type(contract("text"), Shape::Text).unwrap();
    let output = BTreeMap::from([(
        "value".into(),
        Port {
            contract: contract("text"),
            optional: false,
        },
    )]);
    registry
        .register(
            Operation {
                contract: contract("text"),
                implementation: "synthetic/text/v1".into(),
                inputs: ports(),
                outputs: output.clone(),
                resource: Resource::Provider,
                reuse,
            },
            Arc::new(|_, _| {
                Box::pin(async { Ok(BTreeMap::from([("value".into(), json!("text"))])) })
            }),
        )
        .unwrap();
    let mut definition = definition();
    definition.nodes.remove("second");
    definition.nodes.get_mut("first").unwrap().operation = contract("text");
    definition.results.insert("value".into(), source("first"));
    definition.outputs = output;
    Arc::new(registry.compile(definition).unwrap())
}

fn start(host: &mut DurableEngine, store: &mut SqlStore, graph: &Executable, run: &str) {
    store.authorize(run, "scope");
    host.apply(
        Event::Begin {
            run: run.into(),
            artifact: graph.identity().into(),
            inputs: values(40),
            scope: "scope".into(),
            policy: BTreeMap::new(),
        },
        store,
    )
    .unwrap();
}

fn attempt(host: &DurableEngine, run: &str) -> AttemptId {
    host.inspect(run).unwrap().attempts["first"][0].id
}

fn maximum_outcome(execution: ExecutionId, bound: usize) -> Event {
    let empty = Event::Settle {
        execution,
        outcome: Ok(BTreeMap::from([("value".into(), json!(""))])),
    };
    let overhead = serde_json::to_vec(&empty).unwrap().len();
    let event = Event::Settle {
        execution,
        outcome: Ok(BTreeMap::from([(
            "value".into(),
            json!("x".repeat(bound - overhead)),
        )])),
    };
    assert_eq!(serde_json::to_vec(&event).unwrap().len(), bound);
    event
}

fn fill(host: &mut DurableEngine, store: &mut SqlStore, run: &str) -> String {
    for _ in 0..1000 {
        let before = store.bytes();
        match host.apply(
            Event::Pause {
                run: run.into(),
                paused: false,
            },
            store,
        ) {
            Ok(_) => (),
            Err(f) => {
                assert_eq!(store.bytes(), before);
                assert!(!host.poisoned());
                return f.code;
            }
        }
    }
    panic!("fixture never reached its bound")
}

#[test]
fn shared_settlement_adoptions_and_restart_fit_at_the_exact_event_ceiling() {
    let dir = tempfile::tempdir().unwrap();
    let mut store = SqlStore::open(&dir.path().join("owner.db"));
    let graph = graph(Reuse::Exact);
    let limits = limits(100_000, 8);
    let mut host = DurableEngine::create([graph.clone()], limits, &mut store).unwrap();
    start(&mut host, &mut store, &graph, "a");
    start(&mut host, &mut store, &graph, "b");
    let work = host.apply(capacity(1), &mut store).unwrap();
    assert_eq!(work.len(), 1);
    let a = attempt(&host, "a");
    let b = attempt(&host, "b");
    let _invocation = host.claim("a", "first", a, &mut store).unwrap();
    assert_eq!(fill(&mut host, &mut store, "a"), "checkpoint_event_limit");
    let outcome = maximum_outcome(work[0].execution, limits.settlement_event_bytes);
    host.apply(outcome.clone(), &mut store).unwrap();
    host.adopt("b", "first", b, &mut store).unwrap();
    host.adopt("a", "first", a, &mut store).unwrap();
    assert_eq!(store.publications().len(), 2);
    let cp = Checkpoint::decode(&store.bytes(), limits.checkpoint).unwrap();
    let mut host = DurableEngine::recover(cp, [graph.clone()], limits, &mut store).unwrap();
    assert_eq!(host.stamp().revision, 8);
    let bytes = store.bytes();
    for _ in 0..5 {
        let cp = Checkpoint::decode(&bytes, limits.checkpoint).unwrap();
        host = DurableEngine::recover(cp, [graph.clone()], limits, &mut store).unwrap();
        assert_eq!(store.bytes(), bytes);
        assert_eq!(host.stamp().revision, 8);
    }
    let cp = Checkpoint::decode(&bytes, limits.checkpoint).unwrap();
    let replay = cp.replay([graph], state_limits()).unwrap();
    assert!(replay.journal().contains(&outcome));
    assert_eq!(
        host.inspect("a").unwrap().nodes["first"],
        Disposition::Adopted
    );
    assert_eq!(store.publications().len(), 2);
}

#[test]
fn byte_reservations_cover_maximum_results_in_either_producer_order() {
    for reverse in [false, true] {
        let dir = tempfile::tempdir().unwrap();
        let mut store = SqlStore::open(&dir.path().join("owner.db"));
        let graph = graph(Reuse::Fresh);
        let limits = limits(8_000, 1000);
        let mut host = DurableEngine::create([graph.clone()], limits, &mut store).unwrap();
        start(&mut host, &mut store, &graph, "a");
        start(&mut host, &mut store, &graph, "b");
        let mut work = host.apply(capacity(2), &mut store).unwrap();
        assert_eq!(work.len(), 2);
        for run in ["a", "b"] {
            let a = attempt(&host, run);
            let _invocation = host.claim(run, "first", a, &mut store).unwrap();
        }
        assert_eq!(fill(&mut host, &mut store, "a"), "checkpoint_byte_limit");
        if reverse {
            work.reverse();
        }
        for work in work {
            host.apply(
                maximum_outcome(work.execution, limits.settlement_event_bytes),
                &mut store,
            )
            .unwrap();
            assert!(store.bytes().len() <= limits.checkpoint.bytes);
        }
        for run in ["b", "a"] {
            host.adopt(run, "first", attempt(&host, run), &mut store)
                .unwrap();
        }
        let cp = Checkpoint::decode(&store.bytes(), limits.checkpoint).unwrap();
        let host = DurableEngine::recover(cp, [graph], limits, &mut store).unwrap();
        assert!(store.bytes().len() <= limits.checkpoint.bytes);
        assert_eq!(store.publications().len(), 2);
        assert_eq!(
            host.inspect("a").unwrap().nodes["first"],
            Disposition::Adopted
        );
    }
}

#[test]
fn insufficient_reservation_prevents_dispatch_without_returning_an_invocation() {
    for limits in [
        limits(100_000, 4),
        DurableLimits {
            settlement_event_bytes: usize::MAX,
            ..limits(100_000, 100)
        },
    ] {
        let dir = tempfile::tempdir().unwrap();
        let mut store = SqlStore::open(&dir.path().join("owner.db"));
        let graph = graph(Reuse::Exact);
        let mut host = DurableEngine::create([graph.clone()], limits, &mut store).unwrap();
        start(&mut host, &mut store, &graph, "a");
        host.apply(capacity(1), &mut store).unwrap();
        let before = store.bytes();
        let error = host
            .claim("a", "first", attempt(&host, "a"), &mut store)
            .err()
            .unwrap();
        assert!(matches!(
            error.code.as_str(),
            "checkpoint_event_limit" | "checkpoint_byte_limit"
        ));
        assert_eq!(store.bytes(), before);
        assert_eq!(
            host.inspect("a").unwrap().nodes["first"],
            Disposition::Prepared
        );
        let cp = Checkpoint::decode(&before, limits.checkpoint).unwrap();
        let host = DurableEngine::recover(cp, [graph], limits, &mut store).unwrap();
        assert_eq!(
            host.inspect("a").unwrap().nodes["first"],
            Disposition::Failed
        );
    }
}

#[test]
fn cancelled_consumers_do_not_release_the_producer_settlement_reservation() {
    let dir = tempfile::tempdir().unwrap();
    let mut store = SqlStore::open(&dir.path().join("owner.db"));
    let graph = graph(Reuse::Exact);
    let limits = limits(8_000, 1000);
    let mut host = DurableEngine::create([graph.clone()], limits, &mut store).unwrap();
    start(&mut host, &mut store, &graph, "a");
    let work = host.apply(capacity(1), &mut store).unwrap().remove(0);
    let _invocation = host
        .claim("a", "first", attempt(&host, "a"), &mut store)
        .unwrap();
    host.apply(
        Event::Cancel {
            run: "a".into(),
            node: None,
        },
        &mut store,
    )
    .unwrap();
    assert_eq!(fill(&mut host, &mut store, "a"), "checkpoint_byte_limit");
    host.apply(
        maximum_outcome(work.execution, limits.settlement_event_bytes),
        &mut store,
    )
    .unwrap();
    assert!(store.publications().is_empty());
    assert_eq!(
        host.inspect("a").unwrap().nodes["first"],
        Disposition::Cancelled
    );
    let cp = Checkpoint::decode(&store.bytes(), limits.checkpoint).unwrap();
    DurableEngine::recover(cp, [graph], limits, &mut store).unwrap();
}

#[test]
fn oversized_evidence_and_rejected_commits_preserve_the_pending_execution() {
    let dir = tempfile::tempdir().unwrap();
    let mut store = SqlStore::open(&dir.path().join("owner.db"));
    let graph = graph(Reuse::Exact);
    let limits = limits(100_000, 10);
    let mut host = DurableEngine::create([graph.clone()], limits, &mut store).unwrap();
    start(&mut host, &mut store, &graph, "a");
    let work = host.apply(capacity(1), &mut store).unwrap().remove(0);
    let _invocation = host
        .claim("a", "first", attempt(&host, "a"), &mut store)
        .unwrap();
    let before = store.bytes();
    for event in [
        maximum_outcome(work.execution, 257),
        Event::Settle {
            execution: work.execution,
            outcome: Err(unclassified("provider_fixture", &"x".repeat(256))),
        },
    ] {
        assert_eq!(
            host.apply(event.clone(), &mut store).unwrap_err().code,
            "settlement_limit"
        );
        assert_eq!(store.bytes(), before);
        assert_eq!(
            host.inspect("a").unwrap().nodes["first"],
            Disposition::Running
        );
    }
    let outcome = maximum_outcome(work.execution, 256);
    store.fail_before_commit = true;
    assert!(host.apply(outcome.clone(), &mut store).is_err());
    assert_eq!(store.bytes(), before);
    assert!(!host.poisoned());
    store.fail_before_commit = false;
    host.apply(outcome.clone(), &mut store).unwrap();
    let replay = Checkpoint::decode(&store.bytes(), limits.checkpoint)
        .unwrap()
        .replay([graph], state_limits())
        .unwrap();
    assert!(replay.journal().contains(&outcome));
}

#[test]
fn crash_can_consume_its_reserved_slot_without_replaying_external_work() {
    let dir = tempfile::tempdir().unwrap();
    let mut store = SqlStore::open(&dir.path().join("owner.db"));
    let graph = graph(Reuse::Exact);
    let limits = limits(100_000, 6);
    let mut host = DurableEngine::create([graph.clone()], limits, &mut store).unwrap();
    start(&mut host, &mut store, &graph, "a");
    host.apply(capacity(1), &mut store).unwrap();
    let id = attempt(&host, "a");
    let _invocation = host.claim("a", "first", id, &mut store).unwrap();
    assert_eq!(fill(&mut host, &mut store, "a"), "checkpoint_event_limit");
    let cp = Checkpoint::decode(&store.bytes(), limits.checkpoint).unwrap();
    let mut host = DurableEngine::recover(cp, [graph], limits, &mut store).unwrap();
    assert_eq!(
        host.inspect("a").unwrap().nodes["first"],
        Disposition::Unknown
    );
    assert!(host.claim("a", "first", id, &mut store).is_err());
    assert!(store.publications().is_empty());
}

#[test]
fn joining_running_or_retained_work_reserves_each_new_adoption() {
    for retained in [false, true] {
        for enough in [false, true] {
            let dir = tempfile::tempdir().unwrap();
            let mut store = SqlStore::open(&dir.path().join("owner.db"));
            let graph = graph(Reuse::Exact);
            let limits = limits(100_000, if enough { 9 } else { 8 });
            let mut host = DurableEngine::create([graph.clone()], limits, &mut store).unwrap();
            start(&mut host, &mut store, &graph, "a");
            let work = host.apply(capacity(1), &mut store).unwrap().remove(0);
            let a = attempt(&host, "a");
            let _invocation = host.claim("a", "first", a, &mut store).unwrap();
            if retained {
                host.apply(maximum_outcome(work.execution, 256), &mut store)
                    .unwrap();
                host.adopt("a", "first", a, &mut store).unwrap();
            }
            start(&mut host, &mut store, &graph, "b");
            let before = store.bytes();
            let joined = host.apply(capacity(1), &mut store);
            if !enough {
                assert_eq!(joined.unwrap_err().code, "checkpoint_event_limit");
                assert_eq!(store.bytes(), before);
                assert!(host.inspect("b").unwrap().attempts.is_empty());
            } else {
                assert!(joined.unwrap().is_empty());
                let b = attempt(&host, "b");
                assert_eq!(
                    host.inspect("b").unwrap().attempts["first"][0].execution,
                    work.execution
                );
                if !retained {
                    host.apply(maximum_outcome(work.execution, 256), &mut store)
                        .unwrap();
                    host.adopt("a", "first", a, &mut store).unwrap();
                }
                host.adopt("b", "first", b, &mut store).unwrap();
                assert_eq!(store.publications().len(), 2);
                let cp = Checkpoint::decode(&store.bytes(), limits.checkpoint).unwrap();
                let host = DurableEngine::recover(cp, [graph], limits, &mut store).unwrap();
                assert_eq!(host.stamp().revision, 9);
            }
        }
    }
}

#[test]
fn bounded_failure_releases_adoption_capacity_and_retains_original_evidence() {
    let dir = tempfile::tempdir().unwrap();
    let mut store = SqlStore::open(&dir.path().join("owner.db"));
    let graph = graph(Reuse::Exact);
    let limits = limits(100_000, 6);
    let mut host = DurableEngine::create([graph.clone()], limits, &mut store).unwrap();
    start(&mut host, &mut store, &graph, "a");
    let work = host.apply(capacity(1), &mut store).unwrap().remove(0);
    let _invocation = host
        .claim("a", "first", attempt(&host, "a"), &mut store)
        .unwrap();
    let mut event = Event::Settle {
        execution: work.execution,
        outcome: Err(unclassified("fixture_failure", "")),
    };
    let overhead = serde_json::to_vec(&event).unwrap().len();
    if let Event::Settle {
        outcome: Err(f), ..
    } = &mut event
    {
        f.path = "x".repeat(256 - overhead);
    }
    assert_eq!(serde_json::to_vec(&event).unwrap().len(), 256);
    host.apply(event.clone(), &mut store).unwrap();
    assert_eq!(
        host.inspect("a").unwrap().nodes["first"],
        Disposition::Failed
    );
    host.apply(
        Event::Pause {
            run: "a".into(),
            paused: false,
        },
        &mut store,
    )
    .unwrap();
    assert_eq!(fill(&mut host, &mut store, "a"), "checkpoint_event_limit");
    let cp = Checkpoint::decode(&store.bytes(), limits.checkpoint).unwrap();
    let host = DurableEngine::recover(cp, [graph.clone()], limits, &mut store).unwrap();
    assert_eq!(host.stamp().revision, 6);
    let replay = Checkpoint::decode(&store.bytes(), limits.checkpoint)
        .unwrap()
        .replay([graph], state_limits())
        .unwrap();
    assert!(replay.journal().contains(&event));
    assert!(store.publications().is_empty());
}

#[test]
fn unchanged_recovery_checks_actual_stored_bytes_under_new_limits() {
    let dir = tempfile::tempdir().unwrap();
    let mut store = SqlStore::open(&dir.path().join("owner.db"));
    let graph = graph(Reuse::Exact);
    let limits = limits(100_000, 10);
    let mut host = DurableEngine::create([graph.clone()], limits, &mut store).unwrap();
    start(&mut host, &mut store, &graph, "a");
    host.apply(Event::Recover, &mut store).unwrap();
    let compact = store.bytes();
    let value: serde_json::Value = serde_json::from_slice(&compact).unwrap();
    let pretty = serde_json::to_vec_pretty(&value).unwrap();
    assert!(pretty.len() > compact.len());
    store
        .conn
        .execute("UPDATE checkpoint SET payload=?1", [&pretty])
        .unwrap();
    let cp = Checkpoint::decode(&pretty, limits.checkpoint).unwrap();
    let smaller = DurableLimits {
        record_reads: record_read_limits(),
        checkpoint: CheckpointLimits {
            bytes: compact.len(),
            events: 10,
        },
        ..limits
    };
    assert_eq!(
        DurableEngine::recover(cp, [graph], smaller, &mut store)
            .err()
            .unwrap()
            .code,
        "checkpoint_byte_limit"
    );
    assert_eq!(store.bytes(), pretty);
}
