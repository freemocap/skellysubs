use super::{durable_store::SqlStore, *};

fn limits() -> CheckpointLimits {
    CheckpointLimits {
        bytes: 1_000_000,
        events: 100,
    }
}
fn durable_limits() -> DurableLimits {
    DurableLimits {
        record_reads: record_read_limits(),
        checkpoint: limits(),
        settlement_event_bytes: 4096,
        history: history_limits(),
        state: state_limits(),
    }
}
fn start(host: &mut DurableEngine, store: &mut SqlStore, graph: &Executable, run: &str) {
    store.authorize(run, "scope-v1");
    host.apply(
        Event::Begin {
            run: run.into(),
            artifact: graph.identity().into(),
            scope: "scope-v1".into(),
            inputs: values(40),
            policy: BTreeMap::new(),
        },
        store,
    )
    .unwrap();
}
fn attempt(host: &DurableEngine, run: &str) -> AttemptId {
    host.inspect(run).unwrap().attempts["first"][0].id
}

#[tokio::test]
async fn authority_checks_and_publication_share_the_checkpoint_transaction() {
    let dir = tempfile::tempdir().unwrap();
    let mut store = SqlStore::open(&dir.path().join("owner.db"));
    let graph = Arc::new(registry().compile(definition()).unwrap());
    let mut host = DurableEngine::create([graph.clone()], durable_limits(), &mut store).unwrap();
    start(&mut host, &mut store, &graph, "run");
    let work = host.apply(capacity(1), &mut store).unwrap().remove(0);
    let id = attempt(&host, "run");
    let before = store.bytes();
    store.authorize("run", "scope-v2");
    assert_eq!(
        host.claim("run", "first", id, &mut store)
            .err()
            .unwrap()
            .code,
        "authority_changed"
    );
    assert_eq!(store.bytes(), before);
    assert_eq!(
        host.inspect("run").unwrap().nodes["first"],
        Disposition::Prepared
    );
    store.authorize("run", "scope-v1");
    let output = host
        .claim("run", "first", id, &mut store)
        .unwrap()
        .execute(evidence_limits())
        .await
        .outcome
        .unwrap();
    host.apply(
        Event::Settle {
            execution: work.execution,
            outcome: Ok(output.clone()),
        },
        &mut store,
    )
    .unwrap();
    let available = store.bytes();
    store.authorize("run", "scope-v2");
    assert_eq!(
        host.adopt("run", "first", id, &mut store).unwrap_err().code,
        "authority_changed"
    );
    assert!(store.publications().is_empty());
    store.authorize("run", "scope-v1");
    store.fail_before_commit = true;
    assert!(host.adopt("run", "first", id, &mut store).is_err());
    assert_eq!(store.bytes(), available);
    assert!(store.publications().is_empty());
    assert_eq!(
        host.inspect("run").unwrap().nodes["first"],
        Disposition::Available
    );
    store.fail_before_commit = false;
    host.adopt("run", "first", id, &mut store).unwrap();
    assert_eq!(
        store.publications(),
        vec![serde_json::to_string(&output).unwrap()]
    );
    assert!(host.adopt("run", "first", id, &mut store).is_err());
    assert_eq!(store.publications().len(), 1);
}

#[test]
fn crash_recovery_is_durable_and_never_reissues_a_claim() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("owner.db");
    let mut store = SqlStore::open(&path);
    let graph = Arc::new(registry().compile(definition()).unwrap());
    let mut host = DurableEngine::create([graph.clone()], durable_limits(), &mut store).unwrap();
    start(&mut host, &mut store, &graph, "run");
    host.apply(capacity(1), &mut store).unwrap();
    let id = attempt(&host, "run");
    let invocation = host.claim("run", "first", id, &mut store).unwrap();
    let engine_id = host.stamp().engine.clone();
    drop(invocation);
    drop(host);
    drop(store);
    let mut reopened = SqlStore::open(&path);
    let checkpoint = Checkpoint::decode(&reopened.bytes(), limits()).unwrap();
    let mut recovered =
        DurableEngine::recover(checkpoint, [graph], durable_limits(), &mut reopened).unwrap();
    assert_eq!(recovered.stamp().engine, engine_id);
    assert_eq!(
        recovered.inspect("run").unwrap().nodes["first"],
        Disposition::Unknown
    );
    assert!(recovered.claim("run", "first", id, &mut reopened).is_err());
    assert!(
        recovered
            .apply(capacity(1), &mut reopened)
            .unwrap()
            .is_empty()
    );
    assert_eq!(
        Checkpoint::decode(&reopened.bytes(), limits())
            .unwrap()
            .stamp(),
        recovered.stamp()
    );
}

#[tokio::test]
async fn uncertain_adoption_freezes_host_and_reload_does_not_republish() {
    let dir = tempfile::tempdir().unwrap();
    let mut store = SqlStore::open(&dir.path().join("owner.db"));
    let graph = Arc::new(registry().compile(definition()).unwrap());
    let mut host = DurableEngine::create([graph.clone()], durable_limits(), &mut store).unwrap();
    start(&mut host, &mut store, &graph, "run");
    let work = host.apply(capacity(1), &mut store).unwrap().remove(0);
    let id = attempt(&host, "run");
    let outcome = host
        .claim("run", "first", id, &mut store)
        .unwrap()
        .execute(evidence_limits())
        .await
        .outcome;
    host.apply(
        Event::Settle {
            execution: work.execution,
            outcome,
        },
        &mut store,
    )
    .unwrap();
    store.fail_after_commit = true;
    assert_eq!(
        host.adopt("run", "first", id, &mut store).unwrap_err().code,
        "acknowledgment_lost"
    );
    assert!(host.poisoned());
    assert_eq!(
        host.apply(capacity(1), &mut store).unwrap_err().code,
        "reload_required"
    );
    assert!(host.inspect("run").is_err());
    store.fail_after_commit = false;
    let checkpoint = Checkpoint::decode(&store.bytes(), limits()).unwrap();
    let recovered =
        DurableEngine::recover(checkpoint, [graph], durable_limits(), &mut store).unwrap();
    assert_eq!(
        recovered.inspect("run").unwrap().nodes["first"],
        Disposition::Adopted
    );
    assert_eq!(store.publications().len(), 1);
}

#[test]
fn stale_host_and_uncommitted_dispatch_cannot_produce_effects() {
    let dir = tempfile::tempdir().unwrap();
    let mut store = SqlStore::open(&dir.path().join("owner.db"));
    let graph = Arc::new(registry().compile(definition()).unwrap());
    let mut stale = DurableEngine::create([graph.clone()], durable_limits(), &mut store).unwrap();
    start(&mut stale, &mut store, &graph, "run");
    stale.apply(capacity(1), &mut store).unwrap();
    let id = attempt(&stale, "run");
    let before = store.bytes();
    store.fail_before_commit = true;
    assert!(stale.claim("run", "first", id, &mut store).is_err());
    assert_eq!(store.bytes(), before);
    store.fail_before_commit = false;
    let checkpoint = Checkpoint::decode(&before, limits()).unwrap();
    let current =
        DurableEngine::recover(checkpoint, [graph], durable_limits(), &mut store).unwrap();
    assert_eq!(
        stale
            .claim("run", "first", id, &mut store)
            .err()
            .unwrap()
            .code,
        // Typed reads reject the stale revision before the commit CAS.
        "record_stamp"
    );
    assert_eq!(
        Checkpoint::decode(&store.bytes(), limits())
            .unwrap()
            .stamp(),
        current.stamp()
    );
}

#[test]
fn shared_execution_accepts_an_authorized_consumer_and_checks_each_adoption() {
    let dir = tempfile::tempdir().unwrap();
    let mut store = SqlStore::open(&dir.path().join("owner.db"));
    let graph = Arc::new(registry().compile(definition()).unwrap());
    let mut host = DurableEngine::create([graph.clone()], durable_limits(), &mut store).unwrap();
    start(&mut host, &mut store, &graph, "a");
    start(&mut host, &mut store, &graph, "b");
    let work = host.apply(capacity(1), &mut store).unwrap();
    assert_eq!(work.len(), 1);
    let a = attempt(&host, "a");
    let b = attempt(&host, "b");
    store.authorize("a", "changed");
    assert!(host.claim("a", "first", a, &mut store).is_err());
    let _invocation = host.claim("b", "first", b, &mut store).unwrap();
    host.apply(
        Event::Settle {
            execution: work[0].execution,
            outcome: Ok(values(41)),
        },
        &mut store,
    )
    .unwrap();
    assert!(host.adopt("a", "first", a, &mut store).is_err());
    host.adopt("b", "first", b, &mut store).unwrap();
    assert_eq!(store.publications().len(), 1);
}

#[test]
fn malformed_incompatible_and_oversized_checkpoints_fail_without_writes() {
    let dir = tempfile::tempdir().unwrap();
    let mut store = SqlStore::open(&dir.path().join("owner.db"));
    let graph = Arc::new(registry().compile(definition()).unwrap());
    let mut host = DurableEngine::create([graph.clone()], durable_limits(), &mut store).unwrap();
    start(&mut host, &mut store, &graph, "run");
    let original = store.bytes();
    let mut unknown: serde_json::Value = serde_json::from_slice(&original).unwrap();
    unknown["payload"]["events"][0]["Begin"]["future_semantics"] = json!(true);
    assert_eq!(
        Checkpoint::decode(&serde_json::to_vec(&unknown).unwrap(), limits())
            .err()
            .unwrap()
            .code,
        "invalid_checkpoint"
    );
    let mut unknown: serde_json::Value = serde_json::from_slice(&original).unwrap();
    unknown["payload"]["artifacts"][graph.identity()]["definition"]["nodes"]["first"]["future_semantics"] =
        json!(true);
    assert_eq!(
        Checkpoint::decode(&serde_json::to_vec(&unknown).unwrap(), limits())
            .err()
            .unwrap()
            .code,
        "invalid_checkpoint"
    );
    let mut decoded: serde_json::Value = serde_json::from_slice(&original).unwrap();
    decoded["payload"]["events"][0]["Begin"]["inputs"]["value"] = json!(999);
    assert_eq!(
        Checkpoint::decode(&serde_json::to_vec(&decoded).unwrap(), limits())
            .err()
            .unwrap()
            .code,
        "checkpoint_corrupt"
    );
    decoded["payload"]["format"] = json!(999);
    assert_eq!(
        Checkpoint::decode(&serde_json::to_vec(&decoded).unwrap(), limits())
            .err()
            .unwrap()
            .code,
        "checkpoint_version"
    );
    assert_eq!(
        Checkpoint::decode(
            &original,
            CheckpointLimits {
                bytes: 10,
                events: 100
            }
        )
        .err()
        .unwrap()
        .code,
        "checkpoint_byte_limit"
    );
    assert_eq!(
        Checkpoint::decode(
            &original,
            CheckpointLimits {
                bytes: 1_000_000,
                events: 0
            }
        )
        .err()
        .unwrap()
        .code,
        "checkpoint_event_limit"
    );
    let mut different = definition();
    different.contract.version = 2;
    let different = Arc::new(registry().compile(different).unwrap());
    let checkpoint = Checkpoint::decode(&original, limits()).unwrap();
    assert_eq!(
        DurableEngine::recover(checkpoint, [different], durable_limits(), &mut store)
            .err()
            .unwrap()
            .code,
        "checkpoint_artifact_mismatch"
    );
    assert_eq!(store.bytes(), original);
    assert!(
        host.apply(
            Event::Dispatch {
                execution: ExecutionId(10)
            },
            &mut store
        )
        .is_err()
    );
    assert!(
        host.apply(
            Event::Adopt {
                run: "run".into(),
                node: "first".into(),
                attempt: AttemptId(10)
            },
            &mut store
        )
        .is_err()
    );
}

#[test]
fn begin_authority_and_checkpoint_limits_reject_without_changing_history() {
    let dir = tempfile::tempdir().unwrap();
    let mut store = SqlStore::open(&dir.path().join("owner.db"));
    let graph = Arc::new(registry().compile(definition()).unwrap());
    let mut host = DurableEngine::create(
        [graph.clone()],
        DurableLimits {
            record_reads: record_read_limits(),
            checkpoint: CheckpointLimits {
                bytes: 1_000_000,
                events: 2,
            },
            settlement_event_bytes: 4096,
            history: history_limits(),
            state: state_limits(),
        },
        &mut store,
    )
    .unwrap();
    let empty = store.bytes();
    let begin = Event::Begin {
        run: "run".into(),
        artifact: graph.identity().into(),
        scope: "scope-v1".into(),
        inputs: values(40),
        policy: BTreeMap::new(),
    };
    assert_eq!(
        host.apply(begin.clone(), &mut store).unwrap_err().code,
        "authority_changed"
    );
    assert_eq!(store.bytes(), empty);
    store.authorize("run", "scope-v1");
    let mut wrong_source = begin.clone();
    if let Event::Begin { inputs, .. } = &mut wrong_source {
        *inputs = values(999);
    }
    assert_eq!(
        host.apply(wrong_source, &mut store).unwrap_err().code,
        "source_changed"
    );
    assert_eq!(store.bytes(), empty);
    host.apply(begin, &mut store).unwrap();
    let before = store.bytes();
    assert_eq!(
        host.apply(capacity(1), &mut store).unwrap_err().code,
        "checkpoint_event_limit"
    );
    assert_eq!(store.bytes(), before);
    assert_eq!(
        host.inspect("run").unwrap().nodes["first"],
        Disposition::Ready
    );
    assert!(!host.poisoned());
    let checkpoint = Checkpoint::decode(&before, limits()).unwrap();
    let small = CheckpointLimits {
        bytes: before.len(),
        events: 100,
    };
    assert_eq!(
        DurableEngine::recover(
            checkpoint,
            [graph],
            DurableLimits {
                record_reads: record_read_limits(),
                checkpoint: small,
                settlement_event_bytes: 4096,
                history: history_limits(),
                state: state_limits(),
            },
            &mut store
        )
        .err()
        .unwrap()
        .code,
        "checkpoint_byte_limit"
    );
    assert_eq!(store.bytes(), before);
}

#[test]
fn uncertain_dispatch_returns_no_invocation_and_recovers_as_unknown() {
    let dir = tempfile::tempdir().unwrap();
    let mut store = SqlStore::open(&dir.path().join("owner.db"));
    let graph = Arc::new(registry().compile(definition()).unwrap());
    let mut host = DurableEngine::create([graph.clone()], durable_limits(), &mut store).unwrap();
    start(&mut host, &mut store, &graph, "run");
    host.apply(capacity(1), &mut store).unwrap();
    let id = attempt(&host, "run");
    store.fail_after_commit = true;
    assert_eq!(
        host.claim("run", "first", id, &mut store)
            .err()
            .unwrap()
            .code,
        "acknowledgment_lost"
    );
    assert!(host.poisoned());
    store.fail_after_commit = false;
    let checkpoint = Checkpoint::decode(&store.bytes(), limits()).unwrap();
    let recovered =
        DurableEngine::recover(checkpoint, [graph], durable_limits(), &mut store).unwrap();
    assert_eq!(
        recovered.inspect("run").unwrap().nodes["first"],
        Disposition::Unknown
    );
    assert!(store.publications().is_empty());
}
