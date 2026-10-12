use super::{
    durable_store::SqlStore,
    record_access::{Observed, available_host, begin_host, limits},
    *,
};

fn setup(
    store: &mut SqlStore,
    mode: &str,
) -> (DurableEngine, Arc<Executable>, Event, Vec<RecordKey>) {
    let mut definition = definition();
    if mode == "demand" {
        definition.nodes.get_mut("first").unwrap().activation = Activation::OnDemand;
    }
    let graph = Arc::new(registry().compile(definition).unwrap());
    let mut host = DurableEngine::create([graph.clone()], limits(), store).unwrap();
    begin_host(&mut host, store, &graph, "a");
    let mut required = vec![RecordKey::Run("a".into())];
    if mode == "demand" {
        return (host, graph, demand("a", "first"), required);
    }
    if mode == "pause" {
        return (
            host,
            graph,
            Event::Pause {
                run: "a".into(),
                paused: true,
            },
            required,
        );
    }
    begin_host(&mut host, store, &graph, "b");
    let work = host.apply(capacity(1), store).unwrap().remove(0);
    let a = host.inspect("a").unwrap().attempts["first"][0].id;
    let b = host.inspect("b").unwrap().attempts["first"][0].id;
    required.push(RecordKey::Attempt(a));
    if mode != "cancel" {
        drop(host.claim("a", "first", a, store).unwrap());
    }
    if mode == "retry" {
        host.apply(
            Event::Settle {
                execution: work.execution,
                outcome: Err(unclassified("fixture", "provider")),
            },
            store,
        )
        .unwrap();
        return (
            host,
            graph,
            Event::Retry {
                run: "a".into(),
                node: "first".into(),
            },
            required,
        );
    }
    required.extend([
        RecordKey::Execution(work.execution),
        RecordKey::Run("b".into()),
        RecordKey::Attempt(b),
    ]);
    let event = match mode {
        "cancel" => Event::Cancel {
            run: "a".into(),
            node: None,
        },
        "settle" => Event::Settle {
            execution: work.execution,
            outcome: Ok(values(41)),
        },
        "recover" => {
            store.authorize("z", "other");
            host.apply(
                Event::Begin {
                    run: "z".into(),
                    artifact: graph.identity().into(),
                    inputs: values(40),
                    scope: "other".into(),
                    policy: BTreeMap::new(),
                },
                store,
            )
            .unwrap();
            let queued = host.apply(capacity(2), store).unwrap().remove(0);
            required.extend([
                RecordKey::Execution(queued.execution),
                RecordKey::Run("z".into()),
                RecordKey::Attempt(host.inspect("z").unwrap().attempts["first"][0].id),
            ]);
            Event::Recover
        }
        _ => unreachable!(),
    };
    (host, graph, event, required)
}

#[test]
fn every_mutation_reads_its_dependencies_and_rolls_back_on_any_failed_read() {
    for mode in ["demand", "pause", "cancel", "settle", "retry", "recover"] {
        // Observe a successful event and require the domain dependencies explicitly.
        let dir = tempfile::tempdir().unwrap();
        let mut store = SqlStore::open(&dir.path().join("owner.db"));
        let (mut host, _, event, required) = setup(&mut store, mode);
        let mut observed = Observed {
            store: &mut store,
            reads: Vec::new(),
            fail: None,
            commits: 0,
        };
        host.apply(event, &mut observed).unwrap();
        for key in required {
            assert!(observed.reads.contains(&key), "{mode}: {key:?}");
        }
        let mut keys = Vec::new();
        for key in observed.reads {
            if !keys.contains(&key) {
                keys.push(key);
            }
        }
        for key in keys {
            let dir = tempfile::tempdir().unwrap();
            let mut store = SqlStore::open(&dir.path().join("owner.db"));
            let (mut host, graph, event, _) = setup(&mut store, mode);
            let before = store.bytes();
            let cp = Checkpoint::decode(&before, limits().checkpoint).unwrap();
            let (mut replay, _) = cp
                .replay_history([graph], history_limits(), state_limits(), &mut store)
                .unwrap();
            let expected = replay.apply(event.clone()).unwrap();
            let next = cp.capture_next(&replay, limits().checkpoint).unwrap();
            let snapshot = serde_json::to_value(host.inspect("a").unwrap()).unwrap();
            let mut observed = Observed {
                store: &mut store,
                reads: Vec::new(),
                fail: Some(key),
                commits: 0,
            };
            assert_eq!(
                host.apply(event.clone(), &mut observed).unwrap_err().code,
                "read_injected"
            );
            assert_eq!(observed.commits, 0);
            assert_eq!(observed.store.bytes(), before);
            assert_eq!(host.stamp(), cp.stamp());
            assert_eq!(
                serde_json::to_value(host.inspect("a").unwrap()).unwrap(),
                snapshot
            );
            observed.fail = None;
            assert_eq!(host.apply(event, &mut observed).unwrap(), expected);
            assert_eq!(observed.store.bytes(), next.bytes());
            assert_eq!(observed.commits, 1);
        }
    }
}

fn export() -> ExportLimits {
    ExportLimits {
        bytes: 1_000_000,
        attempts: 1000,
    }
}

#[test]
fn stored_inspection_and_outputs_share_native_semantics_and_fail_without_partial_views() {
    for fail in 0..3 {
        let dir = tempfile::tempdir().unwrap();
        let mut store = SqlStore::open(&dir.path().join("owner.db"));
        let (mut host, attempt, execution) = available_host(&mut store);
        host.adopt("a", "first", attempt, &mut store).unwrap();
        let expected =
            serde_json::to_value(host.inspection_snapshot("a", export()).unwrap()).unwrap();
        let keys = [
            RecordKey::Run("a".into()),
            RecordKey::Attempt(attempt),
            RecordKey::Execution(execution),
        ];
        let before = store.bytes();
        let mut observed = Observed {
            store: &mut store,
            reads: Vec::new(),
            fail: Some(keys[fail].clone()),
            commits: 0,
        };
        assert_eq!(
            host.read_inspection("a", export(), &mut observed)
                .err()
                .unwrap()
                .code,
            "read_injected"
        );
        assert_eq!(
            host.read_outputs("a", &mut observed).unwrap_err().code,
            "read_injected"
        );
        assert_eq!(observed.commits, 0);
        assert_eq!(observed.store.bytes(), before);
        observed.fail = None;
        observed.reads.clear();
        let snapshot = host.read_inspection("a", export(), &mut observed).unwrap();
        assert_eq!(snapshot.nodes.len(), 2);
        assert_eq!(snapshot.nodes["second"], Disposition::Ready);
        assert_eq!(serde_json::to_value(snapshot).unwrap(), expected);
        assert_eq!(
            host.read_outputs("a", &mut observed).unwrap(),
            host.outputs("a").unwrap()
        );
        assert!(!observed.reads.contains(&RecordKey::Run("unrelated".into())));
        assert_eq!(observed.commits, 0);
        let work = host.apply(capacity(1), &mut observed).unwrap().remove(0);
        let second = host.inspect("a").unwrap().attempts["second"][0].id;
        drop(host.claim("a", "second", second, &mut observed).unwrap());
        host.apply(
            Event::Settle {
                execution: work.execution,
                outcome: Ok(values(42)),
            },
            &mut observed,
        )
        .unwrap();
        host.adopt("a", "second", second, &mut observed).unwrap();
        assert_eq!(
            host.read_outputs("a", &mut observed).unwrap(),
            Some(values(42))
        );
    }
}

#[test]
fn no_op_recovery_still_checks_required_stored_records_without_committing() {
    let dir = tempfile::tempdir().unwrap();
    let mut store = SqlStore::open(&dir.path().join("owner.db"));
    let (mut host, _, _, _) = setup(&mut store, "recover");
    host.apply(Event::Recover, &mut store).unwrap();
    let before = store.bytes();
    let mut observed = Observed {
        store: &mut store,
        reads: Vec::new(),
        fail: Some(RecordKey::Run("b".into())),
        commits: 0,
    };
    assert_eq!(
        host.apply(Event::Recover, &mut observed).unwrap_err().code,
        "read_injected"
    );
    assert_eq!(observed.commits, 0);
    assert_eq!(observed.store.bytes(), before);
    observed.fail = None;
    assert!(
        host.apply(Event::Recover, &mut observed)
            .unwrap()
            .is_empty()
    );
    assert_eq!(observed.commits, 0);
    assert_eq!(observed.store.bytes(), before);
}
