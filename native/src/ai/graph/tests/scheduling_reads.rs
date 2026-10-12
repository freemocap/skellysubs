use super::{
    durable_store::SqlStore,
    record_access::{Observed, available_host, begin_host, limits},
    *,
};

fn resident_advance(
    store: &mut SqlStore,
    graph: Arc<Executable>,
    event: Event,
) -> (Vec<Work>, Checkpoint) {
    let cp = Checkpoint::decode(&store.bytes(), limits().checkpoint).unwrap();
    let (mut engine, _) = cp
        .replay_history([graph], history_limits(), state_limits(), store)
        .unwrap();
    let work = engine.apply(event).unwrap();
    (work, cp.capture_next(&engine, limits().checkpoint).unwrap())
}

#[test]
fn one_advance_reads_staged_producers_and_subscriptions_without_persisted_rows() {
    let dir = tempfile::tempdir().unwrap();
    let mut store = SqlStore::open(&dir.path().join("owner.db"));
    let graph = Arc::new(registry().compile(definition()).unwrap());
    let mut host = DurableEngine::create([graph.clone()], limits(), &mut store).unwrap();
    for run in ["a", "b"] {
        begin_host(&mut host, &mut store, &graph, run);
    }
    let (expected, checkpoint) = resident_advance(&mut store, graph, capacity(1));
    let mut observed = Observed {
        store: &mut store,
        reads: Vec::new(),
        fail: None,
        commits: 0,
    };
    let work = host.apply(capacity(1), &mut observed).unwrap();
    assert_eq!(work, expected);
    assert_eq!(work.len(), 1);
    assert_eq!(observed.store.bytes(), checkpoint.bytes());
    assert_eq!(observed.commits, 1);
    assert_eq!(
        observed.reads,
        vec![
            RecordKey::Run("a".into()),
            RecordKey::Run("b".into()),
            RecordKey::Run("a".into()),
            RecordKey::Run("b".into())
        ]
    );
    let a = host.inspect("a").unwrap().attempts["first"][0].clone();
    let b = host.inspect("b").unwrap().attempts["first"][0].clone();
    assert_eq!(a.execution, b.execution);
    assert_eq!(a.acquisition, Acquisition::Produced);
    assert_eq!(b.acquisition, Acquisition::Subscribed);
}

#[test]
fn dependency_read_failure_rolls_back_the_entire_advance_including_earlier_allocations() {
    for failure in 0..4 {
        let dir = tempfile::tempdir().unwrap();
        let mut store = SqlStore::open(&dir.path().join("owner.db"));
        let graph = Arc::new(registry().compile(definition()).unwrap());
        let (mut host, attempt, execution) = available_host(&mut store);
        host.adopt("a", "first", attempt, &mut store).unwrap();
        begin_host(&mut host, &mut store, &graph, "z");
        let (expected, checkpoint) = resident_advance(&mut store, graph, capacity(2));
        let keys = [
            RecordKey::Run("a".into()),
            RecordKey::Attempt(attempt),
            RecordKey::Execution(execution),
            RecordKey::Run("z".into()),
        ];
        let before = store.bytes();
        let stamp = host.stamp().clone();
        let usage = host.state_usage().unwrap();
        let publications = store.publications();
        let mut observed = Observed {
            store: &mut store,
            reads: Vec::new(),
            fail: Some(keys[failure].clone()),
            commits: 0,
        };
        assert_eq!(
            host.apply(capacity(2), &mut observed).unwrap_err().code,
            "read_injected"
        );
        assert_eq!(observed.commits, 0);
        assert_eq!(observed.store.bytes(), before);
        assert_eq!(observed.store.publications(), publications);
        assert_eq!(host.stamp(), &stamp);
        assert_eq!(host.state_usage().unwrap(), usage);
        assert_eq!(
            host.inspect("a").unwrap().nodes["second"],
            Disposition::Ready
        );
        assert!(observed.reads.contains(&keys[failure]));
        observed.fail = None;
        assert_eq!(host.apply(capacity(2), &mut observed).unwrap(), expected);
        assert_eq!(observed.store.bytes(), checkpoint.bytes());
        assert_eq!(observed.commits, 1);
    }
}

fn optional_graph() -> Arc<Executable> {
    let mut registry = registry();
    for (name, inputs, outputs, result) in [
        (
            "maybe",
            ports(),
            BTreeMap::from([("value".into(), port(true))]),
            Values::new(),
        ),
        (
            "optional_sink",
            BTreeMap::from([("value".into(), port(true))]),
            ports(),
            values(42),
        ),
    ] {
        registry
            .register(
                Operation {
                    contract: contract(name),
                    implementation: name.into(),
                    inputs,
                    outputs,
                    resource: Resource::Provider,
                    reuse: Reuse::Exact,
                },
                Arc::new(move |_, _| {
                    let result = result.clone();
                    Box::pin(async move { Ok(result) })
                }),
            )
            .unwrap();
    }
    let mut d = definition();
    d.nodes.get_mut("first").unwrap().operation = contract("maybe");
    d.nodes.get_mut("second").unwrap().operation = contract("optional_sink");
    Arc::new(registry.compile(d).unwrap())
}

fn adopted_first(
    store: &mut SqlStore,
    graph: Arc<Executable>,
    outcome: Values,
) -> (DurableEngine, ExecutionId) {
    let mut host = DurableEngine::create([graph.clone()], limits(), store).unwrap();
    begin_host(&mut host, store, &graph, "a");
    let work = host.apply(capacity(1), store).unwrap().remove(0);
    let id = host.inspect("a").unwrap().attempts["first"][0].id;
    drop(host.claim("a", "first", id, store).unwrap());
    host.apply(
        Event::Settle {
            execution: work.execution,
            outcome: Ok(outcome),
        },
        store,
    )
    .unwrap();
    host.adopt("a", "first", id, store).unwrap();
    (host, work.execution)
}

#[tokio::test]
async fn optional_absence_is_a_value_policy_and_never_a_storage_error_fallback() {
    for corruption in ["missing", "oversized", "changed"] {
        let dir = tempfile::tempdir().unwrap();
        let mut store = SqlStore::open(&dir.path().join("owner.db"));
        let graph = optional_graph();
        let (mut host, execution) = adopted_first(&mut store, graph.clone(), Values::new());
        let (expected, checkpoint) = resident_advance(&mut store, graph, capacity(1));
        let key = RecordKey::Execution(execution);
        let original = store
            .read_record(host.stamp(), &key, limits().checkpoint.bytes)
            .unwrap();
        let encoded = serde_json::to_string(&key).unwrap();
        match corruption {
            "missing" => {
                store
                    .conn
                    .execute("DELETE FROM graph_record WHERE key=?1", [&encoded])
                    .unwrap();
            }
            "oversized" => {
                store
                    .conn
                    .execute(
                        "UPDATE graph_record SET payload=zeroblob(1000001) WHERE key=?1",
                        [&encoded],
                    )
                    .unwrap();
            }
            _ => {
                let mut row: serde_json::Value = serde_json::from_slice(&original).unwrap();
                row["value"]["outcome"] = json!(null);
                store
                    .conn
                    .execute(
                        "UPDATE graph_record SET payload=?2 WHERE key=?1",
                        rusqlite::params![encoded, serde_json::to_vec(&row).unwrap()],
                    )
                    .unwrap();
            }
        }
        let before = store.bytes();
        let mut observed = Observed {
            store: &mut store,
            reads: Vec::new(),
            fail: None,
            commits: 0,
        };
        assert_eq!(
            host.apply(capacity(1), &mut observed).unwrap_err().code,
            match corruption {
                "missing" => "record_missing",
                "oversized" => "record_byte_limit",
                _ => "record_mismatch",
            }
        );
        assert_eq!(observed.commits, 0);
        assert_eq!(observed.store.bytes(), before);
        observed
            .store
            .conn
            .execute(
                "INSERT OR REPLACE INTO graph_record VALUES (?1,?2)",
                rusqlite::params![encoded, original],
            )
            .unwrap();
        let work = host.apply(capacity(1), &mut observed).unwrap();
        assert_eq!(work, expected);
        assert_eq!(work.len(), 1);
        assert!(work[0].inputs.is_empty());
        assert_eq!(observed.store.bytes(), checkpoint.bytes());
        let attempt = host.inspect("a").unwrap().attempts["second"][0].id;
        let invocation = host.claim("a", "second", attempt, &mut observed).unwrap();
        assert_eq!(
            invocation.execute(evidence_limits()).await.outcome.unwrap(),
            values(42)
        );
    }
}

#[test]
fn stored_false_guard_skips_but_an_unreadable_guard_result_fails() {
    let mut registry = registry();
    registry
        .register(
            Operation {
                contract: contract("guard"),
                implementation: "guard".into(),
                inputs: ports(),
                outputs: BTreeMap::from([(
                    "value".into(),
                    Port {
                        contract: contract("boolean"),
                        optional: false,
                    },
                )]),
                resource: Resource::Provider,
                reuse: Reuse::Exact,
            },
            Arc::new(|_, _| {
                Box::pin(async { Ok(BTreeMap::from([("value".into(), json!(false))])) })
            }),
        )
        .unwrap();
    let mut d = definition();
    d.nodes.get_mut("first").unwrap().operation = contract("guard");
    let second = d.nodes.get_mut("second").unwrap();
    second.guard = Some(source("first"));
    second
        .inputs
        .insert("value".into(), Source::Input("value".into()));
    d.outputs.get_mut("value").unwrap().optional = true;
    let graph = Arc::new(registry.compile(d).unwrap());
    let dir = tempfile::tempdir().unwrap();
    let mut store = SqlStore::open(&dir.path().join("owner.db"));
    let (mut host, execution) = adopted_first(
        &mut store,
        graph.clone(),
        BTreeMap::from([("value".into(), json!(false))]),
    );
    let (expected, checkpoint) = resident_advance(&mut store, graph, capacity(1));
    let before = store.bytes();
    let mut observed = Observed {
        store: &mut store,
        reads: Vec::new(),
        fail: Some(RecordKey::Execution(execution)),
        commits: 0,
    };
    assert_eq!(
        host.apply(capacity(1), &mut observed).unwrap_err().code,
        "read_injected"
    );
    assert_eq!(observed.commits, 0);
    assert_eq!(observed.store.bytes(), before);
    observed.fail = None;
    assert_eq!(host.apply(capacity(1), &mut observed).unwrap(), expected);
    assert_eq!(observed.store.bytes(), checkpoint.bytes());
    assert_eq!(
        host.inspect("a").unwrap().nodes["second"],
        Disposition::Skipped
    );
}

#[test]
fn reuse_and_resource_admission_propagate_unreadable_producers() {
    for mode in ["queued", "running", "retained", "busy"] {
        let dir = tempfile::tempdir().unwrap();
        let mut store = SqlStore::open(&dir.path().join("owner.db"));
        let graph = Arc::new(registry().compile(definition()).unwrap());
        let mut host = DurableEngine::create([graph.clone()], limits(), &mut store).unwrap();
        begin_host(&mut host, &mut store, &graph, "a");
        let work = host.apply(capacity(1), &mut store).unwrap().remove(0);
        if mode != "queued" {
            let attempt = host.inspect("a").unwrap().attempts["first"][0].id;
            drop(host.claim("a", "first", attempt, &mut store).unwrap());
        }
        if mode == "retained" {
            host.apply(
                Event::Settle {
                    execution: work.execution,
                    outcome: Ok(values(41)),
                },
                &mut store,
            )
            .unwrap();
        }
        let scope = if mode == "busy" { "other" } else { "scope" };
        store.authorize("b", scope);
        host.apply(
            Event::Begin {
                run: "b".into(),
                artifact: graph.identity().into(),
                inputs: values(40),
                scope: scope.into(),
                policy: BTreeMap::new(),
            },
            &mut store,
        )
        .unwrap();
        let (expected, checkpoint) = resident_advance(&mut store, graph, capacity(1));
        let before = store.bytes();
        let mut observed = Observed {
            store: &mut store,
            reads: Vec::new(),
            fail: Some(RecordKey::Execution(work.execution)),
            commits: 0,
        };
        assert_eq!(
            host.apply(capacity(1), &mut observed).unwrap_err().code,
            "read_injected"
        );
        assert_eq!(observed.commits, 0);
        assert_eq!(observed.store.bytes(), before);
        observed.fail = None;
        assert_eq!(host.apply(capacity(1), &mut observed).unwrap(), expected);
        assert_eq!(observed.store.bytes(), checkpoint.bytes());
        if mode == "busy" {
            assert_eq!(host.inspect("b").unwrap().nodes["first"], Disposition::Held);
        }
    }
}
