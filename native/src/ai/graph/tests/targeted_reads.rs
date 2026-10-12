use super::{
    durable_store::SqlStore,
    record_access::{Observed, available_host, begin_host, limits},
    *,
};

#[test]
fn unresolved_output_inputs_retain_disposition_fallback() {
    for mode in ["ready", "disabled", "dormant", "skipped"] {
        let dir = tempfile::tempdir().unwrap();
        let mut store = SqlStore::open(&dir.path().join("owner.db"));
        let mut definition = definition();
        definition.outputs.get_mut("value").unwrap().optional = true;
        let first = definition.nodes.get_mut("first").unwrap();
        match mode {
            "disabled" => first.activation = Activation::Disabled,
            "dormant" => first.activation = Activation::OnDemand,
            "skipped" => {
                first.guard = Some(Source::Constant {
                    contract: contract("boolean"),
                    value: json!(false),
                })
            }
            _ => (),
        }
        definition.nodes.get_mut("second").unwrap().operation = contract("optional");
        let mut registry = registry();
        registry
            .register(
                Operation {
                    contract: contract("optional"),
                    implementation: "synthetic/optional/v1".into(),
                    inputs: BTreeMap::from([("value".into(), port(true))]),
                    outputs: ports(),
                    resource: Resource::Provider,
                    reuse: Reuse::Exact,
                },
                Arc::new(|_, _| Box::pin(async { Ok(values(0)) })),
            )
            .unwrap();
        let graph = Arc::new(registry.compile(definition).unwrap());
        let mut host = DurableEngine::create([graph.clone()], limits(), &mut store).unwrap();
        begin_host(&mut host, &mut store, &graph, "a");
        assert!(host.inspect("a").unwrap().attempts.is_empty());
        assert_eq!(
            host.read_node_inputs("a", &[("second", "value")], &mut store)
                .unwrap(),
            vec![None],
            "{mode}"
        );
    }
}

#[test]
fn pending_retry_preserves_current_failed_output_and_absent_adoption() {
    let dir = tempfile::tempdir().unwrap();
    let mut store = SqlStore::open(&dir.path().join("owner.db"));
    let graph = Arc::new(registry().compile(definition()).unwrap());
    let mut host = DurableEngine::create([graph.clone()], limits(), &mut store).unwrap();
    begin_host(&mut host, &mut store, &graph, "a");
    let work = host.apply(capacity(1), &mut store).unwrap().remove(0);
    let first = host.inspect("a").unwrap().attempts["first"][0].id;
    drop(host.claim("a", "first", first, &mut store).unwrap());
    host.apply(
        Event::Settle {
            execution: work.execution,
            outcome: Err(unclassified("fixture_failure", "provider")),
        },
        &mut store,
    )
    .unwrap();
    host.apply(
        Event::Pause {
            run: "a".into(),
            paused: true,
        },
        &mut store,
    )
    .unwrap();
    host.apply(
        Event::Retry {
            run: "a".into(),
            node: "first".into(),
        },
        &mut store,
    )
    .unwrap();
    assert_eq!(
        host.inspect("a").unwrap().nodes["first"],
        Disposition::Paused
    );
    assert_eq!(
        host.read_adopted_attempt("a", "first", &mut store).unwrap(),
        None
    );
    assert_eq!(
        host.read_node_input("a", "second", "value", &mut store)
            .unwrap(),
        None
    );
}

#[test]
fn adopted_identity_reads_only_its_owner_and_current_attempt() {
    let dir = tempfile::tempdir().unwrap();
    let mut store = SqlStore::open(&dir.path().join("owner.db"));
    let (mut host, first, _) = available_host(&mut store);
    host.adopt("a", "first", first, &mut store).unwrap();
    host.apply(capacity(4), &mut store).unwrap();
    let second = host.inspect("a").unwrap().attempts["second"]
        .last()
        .unwrap()
        .id;
    let mut observed = Observed {
        store: &mut store,
        reads: Vec::new(),
        fail: Some(RecordKey::Attempt(second)),
        commits: 0,
    };
    assert_eq!(
        host.read_adopted_attempt("a", "first", &mut observed)
            .unwrap(),
        Some(first)
    );
    assert_eq!(
        observed.reads,
        vec![RecordKey::Run("a".into()), RecordKey::Attempt(first)]
    );
    for key in [RecordKey::Run("a".into()), RecordKey::Attempt(first)] {
        observed.fail = Some(key);
        assert_eq!(
            host.read_adopted_attempt("a", "first", &mut observed)
                .unwrap_err()
                .code,
            "read_injected"
        );
    }
    observed.fail = None;
    assert_eq!(
        host.read_adopted_attempt("a", "second", &mut observed)
            .unwrap(),
        None
    );
    assert_eq!(
        host.read_adopted_attempt("a", "absent", &mut observed)
            .unwrap(),
        None
    );
    assert_eq!(observed.commits, 0);
}

#[test]
fn resolved_inputs_do_not_inspect_unrelated_attempts() {
    let dir = tempfile::tempdir().unwrap();
    let mut store = SqlStore::open(&dir.path().join("owner.db"));
    let (mut host, first, execution) = available_host(&mut store);
    host.adopt("a", "first", first, &mut store).unwrap();
    host.apply(capacity(4), &mut store).unwrap();
    let second = host.inspect("a").unwrap().attempts["second"]
        .last()
        .unwrap()
        .id;
    let mut observed = Observed {
        store: &mut store,
        reads: Vec::new(),
        fail: Some(RecordKey::Attempt(second)),
        commits: 0,
    };
    assert_eq!(
        host.read_node_inputs(
            "a",
            &[("first", "value"), ("second", "value")],
            &mut observed
        )
        .unwrap(),
        vec![Some(json!(40)), Some(json!(41))]
    );
    assert_eq!(
        observed.reads,
        vec![
            RecordKey::Run("a".into()),
            RecordKey::Attempt(first),
            RecordKey::Execution(execution)
        ]
    );
    observed.fail = Some(RecordKey::Execution(execution));
    assert_eq!(
        host.read_node_input("a", "second", "value", &mut observed)
            .unwrap_err()
            .code,
        "read_injected"
    );
    assert_eq!(observed.commits, 0);
}

#[test]
fn targeted_adoption_matches_inspection_across_control_changes_and_eviction() {
    let dir = tempfile::tempdir().unwrap();
    let mut store = SqlStore::open(&dir.path().join("owner.db"));
    let (mut host, first, _) = available_host(&mut store);
    let check = |host: &DurableEngine, store: &mut SqlStore| {
        let view = host.inspect("a").unwrap();
        for (node, state) in &view.nodes {
            let expected = if *state == Disposition::Adopted {
                Some(view.attempts[node].last().unwrap().id)
            } else {
                None
            };
            assert_eq!(
                host.read_adopted_attempt("a", node, store).unwrap(),
                expected
            );
        }
    };
    check(&host, &mut store);
    host.adopt("a", "first", first, &mut store).unwrap();
    check(&host, &mut store);
    host.apply(
        Event::Pause {
            run: "a".into(),
            paused: true,
        },
        &mut store,
    )
    .unwrap();
    check(&host, &mut store);
    host.apply(
        Event::Cancel {
            run: "a".into(),
            node: None,
        },
        &mut store,
    )
    .unwrap();
    check(&host, &mut store);
    let expected = host.read_adopted_attempt("a", "first", &mut store).unwrap();
    host.evict_records(&mut store).unwrap();
    assert_eq!(
        host.read_adopted_attempt("a", "first", &mut store).unwrap(),
        expected
    );
}

#[test]
fn one_preview_does_not_read_an_unrelated_current_producer() {
    let dir = tempfile::tempdir().unwrap();
    let mut store = SqlStore::open(&dir.path().join("owner.db"));
    let (mut host, first, _) = available_host(&mut store);
    host.adopt("a", "first", first, &mut store).unwrap();
    host.apply(capacity(4), &mut store).unwrap();
    let second = host.inspect("a").unwrap().attempts["second"]
        .last()
        .unwrap()
        .id;
    let mut observed = Observed {
        store: &mut store,
        reads: Vec::new(),
        fail: Some(RecordKey::Attempt(second)),
        commits: 0,
    };
    assert!(
        host.read_node_preview("a", "first", 100_000, &mut observed)
            .unwrap()
            .is_none()
    );
    assert_eq!(observed.reads.len(), 3);
    assert_eq!(observed.commits, 0);
    observed.fail = Some(RecordKey::Attempt(first));
    assert_eq!(
        host.read_node_preview("a", "first", 100_000, &mut observed)
            .unwrap_err()
            .code,
        "read_injected"
    );
}
