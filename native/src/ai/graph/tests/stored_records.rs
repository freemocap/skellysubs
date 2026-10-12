use super::{durable_store::SqlStore, *};

fn limits() -> DurableLimits {
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

fn start(host: &mut DurableEngine, graph: &Executable, store: &mut SqlStore, run: &str) {
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

fn rows(store: &SqlStore) -> Vec<(String, Vec<u8>)> {
    store
        .conn
        .prepare("SELECT key,payload FROM graph_record ORDER BY key")
        .unwrap()
        .query_map([], |r| Ok((r.get(0)?, r.get(1)?)))
        .unwrap()
        .collect::<std::result::Result<_, _>>()
        .unwrap()
}

#[tokio::test]
async fn native_record_upserts_share_dispatch_and_publication_transactions() {
    let dir = tempfile::tempdir().unwrap();
    let mut store = SqlStore::open(&dir.path().join("owner.db"));
    let graph = Arc::new(registry().compile(definition()).unwrap());
    let mut host = DurableEngine::create([graph.clone()], limits(), &mut store).unwrap();
    start(&mut host, &graph, &mut store, "a");
    assert_eq!(store.last_record_writes, vec![RecordKey::Run("a".into())]);
    start(&mut host, &graph, &mut store, "b");
    let work = host.apply(capacity(1), &mut store).unwrap().remove(0);
    assert_eq!(store.last_record_writes.len(), 5); // two runs/attempts, one producer
    let a = host.inspect("a").unwrap().attempts["first"][0].id;
    let before = rows(&store);
    let checkpoint = store.bytes();
    store.fail_record_after = Some(1);
    assert!(host.claim("a", "first", a, &mut store).is_err());
    assert_eq!(rows(&store), before);
    assert_eq!(store.bytes(), checkpoint);
    assert_eq!(
        host.inspect("a").unwrap().nodes["first"],
        Disposition::Prepared
    );
    store.fail_record_after = None;
    let output = host
        .claim("a", "first", a, &mut store)
        .unwrap()
        .execute(evidence_limits())
        .await
        .outcome;
    assert_eq!(store.last_record_writes.len(), 3); // two attempts, one producer
    assert!(
        !store
            .last_record_writes
            .iter()
            .any(|k| matches!(k, RecordKey::Run(_)))
    );
    host.apply(
        Event::Settle {
            execution: work.execution,
            outcome: output,
        },
        &mut store,
    )
    .unwrap();
    let before = rows(&store);
    let checkpoint = store.bytes();
    store.fail_record_after = Some(1);
    assert!(host.adopt("a", "first", a, &mut store).is_err());
    assert_eq!(rows(&store), before);
    assert_eq!(store.bytes(), checkpoint);
    assert!(store.publications().is_empty());
    store.fail_record_after = None;
    host.adopt("a", "first", a, &mut store).unwrap();
    assert_eq!(store.last_record_writes, vec![RecordKey::Attempt(a)]);
    assert_eq!(store.publications().len(), 1);
    let before = rows(&store);
    host.compact(&mut store).unwrap();
    assert!(store.last_record_writes.is_empty());
    assert_eq!(rows(&store), before);
    let cp = Checkpoint::decode(&store.bytes(), limits().checkpoint).unwrap();
    let host = DurableEngine::recover(cp, [graph], limits(), &mut store).unwrap();
    assert_eq!(
        host.inspect("a").unwrap().nodes["first"],
        Disposition::Adopted
    );
    assert_eq!(store.publications().len(), 1);
}

#[test]
fn record_reads_are_bounded_stamp_bound_and_audited_before_recovery_writes() {
    for corruption in ["missing", "changed", "extra", "oversized"] {
        let dir = tempfile::tempdir().unwrap();
        let mut store = SqlStore::open(&dir.path().join("owner.db"));
        let graph = Arc::new(registry().compile(definition()).unwrap());
        let mut host = DurableEngine::create([graph.clone()], limits(), &mut store).unwrap();
        start(&mut host, &graph, &mut store, "a");
        let key = RecordKey::Run("a".into());
        let stamp = host.stamp().clone();
        let encoded_key = serde_json::to_string(&key).unwrap();
        assert_eq!(
            store.read_record(&stamp, &key, 1).unwrap_err().code,
            "record_byte_limit"
        );
        assert_eq!(
            store
                .read_record(&stamp, &RecordKey::Run("missing".into()), 1000)
                .unwrap_err()
                .code,
            "record_missing"
        );
        let mut stale = stamp.clone();
        stale.revision += 1;
        assert_eq!(
            store.read_record(&stale, &key, 1000).unwrap_err().code,
            "record_stamp"
        );
        match corruption {
            "missing" => {
                store.conn.execute("DELETE FROM graph_record", []).unwrap();
            }
            "extra" => {
                store
                    .conn
                    .execute("INSERT INTO graph_record VALUES ('extra',x'00')", [])
                    .unwrap();
            }
            "oversized" => {
                store
                    .conn
                    .execute("UPDATE graph_record SET payload=zeroblob(1000001)", [])
                    .unwrap();
            }
            _ => {
                store
                    .conn
                    .execute(
                        "UPDATE graph_record SET payload=x'00' WHERE key=?1",
                        [encoded_key],
                    )
                    .unwrap();
            }
        }
        let before = rows(&store);
        let bytes = store.bytes();
        let cp = Checkpoint::decode(&bytes, limits().checkpoint).unwrap();
        let failure = DurableEngine::recover(cp, [graph], limits(), &mut store)
            .err()
            .unwrap();
        assert_eq!(
            failure.code,
            if corruption == "oversized" {
                "record_byte_limit"
            } else {
                "record_mismatch"
            }
        );
        assert_eq!(rows(&store), before);
        assert_eq!(store.bytes(), bytes);
        assert!(store.publications().is_empty());
    }
}

#[test]
fn lost_record_commit_acknowledgment_requires_reload_and_noop_writes_are_empty() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("owner.db");
    let mut store = SqlStore::open(&path);
    let graph = Arc::new(registry().compile(definition()).unwrap());
    let mut host = DurableEngine::create([graph.clone()], limits(), &mut store).unwrap();
    start(&mut host, &graph, &mut store, "a");
    host.apply(
        Event::Pause {
            run: "a".into(),
            paused: false,
        },
        &mut store,
    )
    .unwrap();
    assert!(store.last_record_writes.is_empty());
    let old = host.stamp().clone();
    store.fail_after_commit = true;
    assert_eq!(
        host.apply(capacity(1), &mut store).unwrap_err().code,
        "acknowledgment_lost"
    );
    assert!(host.poisoned());
    assert!(host.inspect("a").is_err());
    assert_eq!(store.record_count(&old).unwrap_err().code, "record_stamp");
    assert_eq!(rows(&store).len(), 3);
    drop(host);
    drop(store);
    let mut store = SqlStore::open(&path);
    let cp = Checkpoint::decode(&store.bytes(), limits().checkpoint).unwrap();
    let mut host = DurableEngine::recover(cp, [graph], limits(), &mut store).unwrap();
    assert_eq!(
        host.inspect("a").unwrap().nodes["first"],
        Disposition::Failed
    );
    assert!(host.apply(capacity(1), &mut store).unwrap().is_empty());
    assert!(store.publications().is_empty());
    assert_eq!(rows(&store).len(), 3);
}
