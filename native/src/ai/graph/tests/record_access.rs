use super::{durable_store::SqlStore, *};
use crate::ai::graph::{
    record_access::{RecordAccess, StoredAccess},
    state::RuntimeState,
};

pub(super) fn limits() -> DurableLimits {
    DurableLimits {
        record_reads: record_read_limits(),
        checkpoint: CheckpointLimits {
            bytes: 1_000_000,
            events: 1000,
        },
        settlement_event_bytes: 4096,
        history: history_limits(),
        state: state_limits(),
    }
}

pub(super) struct Observed<'a> {
    pub store: &'a mut SqlStore,
    pub reads: Vec<RecordKey>,
    pub fail: Option<RecordKey>,
    pub commits: usize,
}
impl RecordStore for Observed<'_> {
    fn record_count(&mut self, stamp: &Stamp) -> Result<usize> {
        self.store.record_count(stamp)
    }
    fn read_record(&mut self, stamp: &Stamp, key: &RecordKey, max: usize) -> Result<Vec<u8>> {
        self.reads.push(key.clone());
        if self.fail.as_ref() == Some(key) {
            return Err(unclassified("read_injected", "record"));
        }
        self.store.read_record(stamp, key, max)
    }
}
impl HistoryStore for Observed<'_> {
    fn read_archive(&mut self, stamp: &Stamp, max: usize) -> Result<Vec<u8>> {
        self.store.read_archive(stamp, max)
    }
}
impl CommitStore for Observed<'_> {
    fn commit(&mut self, request: CommitRequest<'_>) -> std::result::Result<(), CommitFailure> {
        self.commits += 1;
        self.store.commit(request)
    }
}

pub(super) fn begin_host(
    host: &mut DurableEngine,
    store: &mut SqlStore,
    graph: &Executable,
    run: &str,
) {
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

pub(super) fn available_host(store: &mut SqlStore) -> (DurableEngine, AttemptId, ExecutionId) {
    let graph = Arc::new(registry().compile(definition()).unwrap());
    let mut host = DurableEngine::create([graph.clone()], limits(), store).unwrap();
    begin_host(&mut host, store, &graph, "a");
    let work = host.apply(capacity(1), store).unwrap().remove(0);
    let attempt = host.inspect("a").unwrap().attempts["first"][0].id;
    drop(host.claim("a", "first", attempt, store).unwrap());
    host.apply(
        Event::Settle {
            execution: work.execution,
            outcome: Ok(values(41)),
        },
        store,
    )
    .unwrap();
    begin_host(&mut host, store, &graph, "unrelated");
    (host, attempt, work.execution)
}

#[test]
fn protected_input_read_verifies_each_record_once_and_never_reuses_a_previous_read() {
    let dir = tempfile::tempdir().unwrap();
    let mut store = SqlStore::open(&dir.path().join("owner.db"));
    let (mut host, attempt, _) = available_host(&mut store);
    host.adopt("a", "first", attempt, &mut store).unwrap();
    let mut observed = Observed {
        store: &mut store,
        reads: Vec::new(),
        fail: None,
        commits: 0,
    };
    assert_eq!(
        host.read_node_input("a", "second", "value", &mut observed)
            .unwrap(),
        Some(json!(41))
    );
    let unique: std::collections::BTreeSet<_> = observed.reads.iter().collect();
    assert_eq!(
        observed.reads.len(),
        unique.len(),
        "one protected read repeatedly loads the same records"
    );
    observed.fail = Some(RecordKey::Attempt(attempt));
    assert_eq!(
        host.read_node_input("a", "second", "value", &mut observed)
            .unwrap_err()
            .code,
        "read_injected"
    );
    assert_eq!(observed.commits, 0);
}

#[test]
fn durable_adoption_reads_only_its_three_native_records_and_never_falls_back() {
    for missing_at in 0..3 {
        let dir = tempfile::tempdir().unwrap();
        let mut store = SqlStore::open(&dir.path().join("owner.db"));
        let (mut host, attempt, execution) = available_host(&mut store);
        let keys = vec![
            RecordKey::Run("a".into()),
            RecordKey::Attempt(attempt),
            RecordKey::Execution(execution),
        ];
        let bytes = store.bytes();
        let revision = host.stamp().clone();
        let mut observed = Observed {
            store: &mut store,
            reads: Vec::new(),
            fail: Some(keys[missing_at].clone()),
            commits: 0,
        };
        assert_eq!(
            host.adopt("a", "first", attempt, &mut observed)
                .unwrap_err()
                .code,
            "read_injected"
        );
        assert_eq!(observed.reads, keys[..=missing_at]);
        assert_eq!(observed.commits, 0);
        assert_eq!(observed.store.bytes(), bytes);
        assert!(observed.store.publications().is_empty());
        assert_eq!(host.stamp(), &revision);
        assert_eq!(
            host.inspect("a").unwrap().nodes["first"],
            Disposition::Available
        );
        observed.fail = None;
        observed.reads.clear();
        host.adopt("a", "first", attempt, &mut observed).unwrap();
        assert_eq!(observed.reads, keys);
        assert_eq!(observed.commits, 1);
        assert_eq!(
            observed.store.publications(),
            vec![serde_json::to_string(&values(41)).unwrap()]
        );
    }
}

#[test]
fn batched_inputs_share_verified_records_and_preserve_unresolved_values() {
    let dir = tempfile::tempdir().unwrap();
    let mut store = SqlStore::open(&dir.path().join("owner.db"));
    let (mut host, attempt, _) = available_host(&mut store);
    let inputs = [("first", "value"), ("second", "value"), ("second", "value")];
    assert_eq!(
        host.read_node_inputs("a", &inputs, &mut store).unwrap(),
        vec![Some(json!(40)), None, None]
    );
    host.adopt("a", "first", attempt, &mut store).unwrap();
    let mut observed = Observed {
        store: &mut store,
        reads: Vec::new(),
        fail: None,
        commits: 0,
    };
    assert_eq!(
        host.read_node_inputs("a", &inputs, &mut observed).unwrap(),
        vec![Some(json!(40)), Some(json!(41)), Some(json!(41))]
    );
    assert_eq!(observed.reads.len(), 3);
    observed.fail = Some(RecordKey::Attempt(attempt));
    assert_eq!(
        host.read_node_inputs("a", &inputs, &mut observed)
            .unwrap_err()
            .code,
        "read_injected"
    );
    assert_eq!(observed.commits, 0);
}

#[test]
fn adoption_rejects_changed_missing_and_oversized_stored_dependencies_without_writes() {
    for kind in 0..3 {
        for corruption in ["changed", "missing", "oversized"] {
            let dir = tempfile::tempdir().unwrap();
            let mut store = SqlStore::open(&dir.path().join("owner.db"));
            let (mut host, attempt, execution) = available_host(&mut store);
            let keys = [
                RecordKey::Run("a".into()),
                RecordKey::Attempt(attempt),
                RecordKey::Execution(execution),
            ];
            let key = serde_json::to_string(&keys[kind]).unwrap();
            let original = store
                .read_record(host.stamp(), &keys[kind], limits().checkpoint.bytes)
                .unwrap();
            match corruption {
                "missing" => {
                    store
                        .conn
                        .execute("DELETE FROM graph_record WHERE key=?1", [&key])
                        .unwrap();
                }
                "oversized" => {
                    store
                        .conn
                        .execute(
                            "UPDATE graph_record SET payload=zeroblob(1000001) WHERE key=?1",
                            [&key],
                        )
                        .unwrap();
                }
                _ => {
                    let mut value: serde_json::Value = serde_json::from_slice(&original).unwrap();
                    match kind {
                        0 => value["value"]["scope"] = json!("changed"),
                        1 => value["value"]["run"] = json!("unrelated"),
                        _ => value["value"]["outcome"]["Ok"]["value"] = json!(999),
                    }
                    store
                        .conn
                        .execute(
                            "UPDATE graph_record SET payload=?2 WHERE key=?1",
                            rusqlite::params![key, serde_json::to_vec(&value).unwrap()],
                        )
                        .unwrap();
                }
            }
            let before = store.bytes();
            let error = host.adopt("a", "first", attempt, &mut store).unwrap_err();
            assert_eq!(
                error.code,
                match corruption {
                    "missing" => "record_missing",
                    "oversized" => "record_byte_limit",
                    _ => "record_mismatch",
                }
            );
            assert_eq!(store.bytes(), before);
            assert!(store.publications().is_empty());
            assert_eq!(
                host.inspect("a").unwrap().nodes["first"],
                Disposition::Available
            );
        }
    }
}

#[test]
fn typed_readers_share_native_shapes_but_stored_reads_return_decoded_rows() {
    let graph = Arc::new(registry().compile(definition()).unwrap());
    // Use the same native state shape as persistence, without a parallel DTO.
    let dir = tempfile::tempdir().unwrap();
    let mut store = SqlStore::open(&dir.path().join("owner.db"));
    let (host, attempt, execution) = available_host(&mut store);
    let cp = Checkpoint::decode(&store.bytes(), limits().checkpoint).unwrap();
    let (engine, _) = cp
        .replay_history([graph], history_limits(), state_limits(), &mut store)
        .unwrap();
    let state: &RuntimeState = &engine.state;
    let resident = RecordAccess::run(&mut &engine.state, "a").unwrap();
    assert!(std::ptr::eq(resident.as_ref(), &state.runs["a"]));
    let evidence = crate::ai::graph::record_evidence::RecordEvidence::from_state(
        state,
        host.stamp(),
        limits().checkpoint.bytes,
    )
    .unwrap();
    let mut stored = StoredAccess {
        evidence: &evidence,
        max_bytes: limits().checkpoint.bytes,
        store: &mut store,
    };
    let decoded = stored.run("a").unwrap();
    assert_eq!(decoded, resident);
    assert!(!Arc::ptr_eq(&decoded, &resident));
    assert_eq!(
        stored.attempt(attempt).unwrap().as_ref(),
        &state.attempts[&attempt]
    );
    assert_eq!(
        stored.execution(execution).unwrap().as_ref(),
        &state.executions[&execution]
    );
    let decision = engine.adoption(&mut stored, "a", "first", attempt).unwrap();
    assert_eq!(
        decision.values(),
        &engine.available("a", "first", attempt).unwrap()
    );
}

fn dispatch_host(store: &mut SqlStore) -> (DurableEngine, AttemptId, Vec<RecordKey>) {
    let graph = Arc::new(registry().compile(definition()).unwrap());
    let mut host = DurableEngine::create([graph.clone()], limits(), store).unwrap();
    begin_host(&mut host, store, &graph, "a");
    begin_host(&mut host, store, &graph, "b");
    store.authorize("busy", "other-scope");
    host.apply(
        Event::Begin {
            run: "busy".into(),
            artifact: graph.identity().into(),
            inputs: values(40),
            scope: "other-scope".into(),
            policy: BTreeMap::new(),
        },
        store,
    )
    .unwrap();
    host.apply(capacity(2), store).unwrap();
    let a = host.inspect("a").unwrap().attempts["first"][0].clone();
    let b = host.inspect("b").unwrap().attempts["first"][0].id;
    let busy = host.inspect("busy").unwrap().attempts["first"][0].clone();
    drop(host.claim("busy", "first", busy.id, store).unwrap());
    let reads = vec![
        RecordKey::Run("a".into()),
        RecordKey::Attempt(a.id),
        RecordKey::Execution(a.execution),
        RecordKey::Attempt(a.id),
        RecordKey::Run("a".into()),
        RecordKey::Attempt(b),
        RecordKey::Run("b".into()),
        RecordKey::Execution(busy.execution),
    ];
    (host, a.id, reads)
}

#[tokio::test]
async fn dispatch_reads_shared_consumers_and_capacity_before_commit_or_invocation() {
    for missing_at in [0, 1, 2, 5, 6, 7] {
        let dir = tempfile::tempdir().unwrap();
        let mut store = SqlStore::open(&dir.path().join("owner.db"));
        let (mut host, attempt, keys) = dispatch_host(&mut store);
        let before = store.bytes();
        let stamp = host.stamp().clone();
        let mut observed = Observed {
            store: &mut store,
            reads: Vec::new(),
            fail: Some(keys[missing_at].clone()),
            commits: 0,
        };
        let error = host
            .claim("a", "first", attempt, &mut observed)
            .err()
            .unwrap();
        assert_eq!(error.code, "read_injected");
        assert_eq!(observed.reads, keys[..=missing_at]);
        assert_eq!(observed.commits, 0);
        assert_eq!(observed.store.bytes(), before);
        assert_eq!(host.stamp(), &stamp);
        for run in ["a", "b"] {
            assert_eq!(
                host.inspect(run).unwrap().nodes["first"],
                Disposition::Prepared
            );
        }
        observed.fail = None;
        observed.reads.clear();
        let invocation = host.claim("a", "first", attempt, &mut observed).unwrap();
        assert_eq!(observed.reads, keys);
        assert_eq!(observed.commits, 1);
        assert_eq!(
            invocation.execute(evidence_limits()).await.outcome.unwrap(),
            values(41)
        );
        for run in ["a", "b", "busy"] {
            assert_eq!(
                host.inspect(run).unwrap().nodes["first"],
                Disposition::Running
            );
        }
        assert!(host.claim("a", "first", attempt, &mut observed).is_err());
        assert_eq!(observed.commits, 1);
    }
}

#[test]
fn dispatch_rejects_corrupt_consumer_and_capacity_rows_before_writes() {
    for index in [0, 1, 2, 5, 6, 7] {
        for corruption in ["missing", "oversized", "changed"] {
            let dir = tempfile::tempdir().unwrap();
            let mut store = SqlStore::open(&dir.path().join("owner.db"));
            let (mut host, attempt, keys) = dispatch_host(&mut store);
            let key = serde_json::to_string(&keys[index]).unwrap();
            match corruption {
                "missing" => {
                    store
                        .conn
                        .execute("DELETE FROM graph_record WHERE key=?1", [&key])
                        .unwrap();
                }
                "oversized" => {
                    store
                        .conn
                        .execute(
                            "UPDATE graph_record SET payload=zeroblob(1000001) WHERE key=?1",
                            [&key],
                        )
                        .unwrap();
                }
                _ => {
                    let bytes = store
                        .read_record(host.stamp(), &keys[index], limits().checkpoint.bytes)
                        .unwrap();
                    let mut row: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
                    match keys[index] {
                        RecordKey::Run(_) => row["value"]["paused"] = json!(true),
                        RecordKey::Attempt(_) => {
                            row["value"]["attempt"]["state"] = json!("Cancelled")
                        }
                        RecordKey::Execution(_) => {
                            row["value"]["work"]["inputs"] = json!(values(999))
                        }
                    }
                    store
                        .conn
                        .execute(
                            "UPDATE graph_record SET payload=?2 WHERE key=?1",
                            rusqlite::params![key, serde_json::to_vec(&row).unwrap()],
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
            let error = host
                .claim("a", "first", attempt, &mut observed)
                .err()
                .unwrap();
            assert_eq!(
                error.code,
                match corruption {
                    "missing" => "record_missing",
                    "oversized" => "record_byte_limit",
                    _ => "record_mismatch",
                }
            );
            assert_eq!(observed.commits, 0);
            assert_eq!(observed.store.bytes(), before);
            assert_eq!(
                host.inspect("a").unwrap().nodes["first"],
                Disposition::Prepared
            );
            assert_eq!(
                host.inspect("b").unwrap().nodes["first"],
                Disposition::Prepared
            );
        }
    }
}
