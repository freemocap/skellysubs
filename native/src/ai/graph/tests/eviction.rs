use super::{
    durable_store::SqlStore,
    record_access::{Observed, begin_host, limits},
    *,
};

fn export() -> ExportLimits {
    ExportLimits {
        bytes: 1_000_000,
        attempts: 1000,
    }
}

fn attempt(host: &DurableEngine, store: &mut SqlStore, run: &str, node: &str) -> AttemptId {
    AttemptId(
        host.read_inspection(run, export(), store).unwrap().attempts[node]
            .last()
            .unwrap()
            .id
            .parse()
            .unwrap(),
    )
}

fn evict(host: &mut DurableEngine, store: &mut SqlStore, runs: &[&str]) {
    let before: Vec<_> = runs
        .iter()
        .map(|run| {
            serde_json::to_value(host.read_inspection(run, export(), store).unwrap()).unwrap()
        })
        .collect();
    let logical = host.state_usage().unwrap();
    let rows = store.record_count(host.stamp()).unwrap();
    let revision = host.stamp().revision;
    host.evict_records(store).unwrap();
    assert_eq!(
        host.resident_usage().unwrap(),
        StateUsage {
            runs: 0,
            attempts: 0,
            executions: 0
        }
    );
    assert_eq!(host.state_usage().unwrap(), logical);
    assert_eq!(host.stamp().revision, revision);
    assert_eq!(store.record_count(host.stamp()).unwrap(), rows);
    for (run, expected) in runs.iter().zip(before) {
        assert_eq!(
            serde_json::to_value(host.read_inspection(run, export(), store).unwrap()).unwrap(),
            expected
        );
        assert_eq!(host.inspect(run).err().unwrap().code, "record_not_resident");
        assert_eq!(host.outputs(run).unwrap_err().code, "record_not_resident");
    }
    let current: serde_json::Value = serde_json::from_slice(&store.bytes()).unwrap();
    assert_eq!(current["payload"]["format"], 4);
    assert_eq!(current["payload"]["events"], json!([]));
    let state = &current["payload"]["base"]["state"];
    assert_eq!(state.as_object().unwrap().len(), 4);
    assert!(state.get("rows").unwrap().is_array());
    assert!(state.get("runs").is_none());
    let stamp = host.stamp().clone();
    host.evict_records(store).unwrap();
    assert_eq!(host.stamp(), &stamp);
}

fn replay_matches(
    host: &DurableEngine,
    store: &mut SqlStore,
    graph: Arc<Executable>,
    runs: &[&str],
) {
    let cp = Checkpoint::decode(&store.bytes(), limits().checkpoint).unwrap();
    let (engine, _) = cp
        .replay_history([graph], history_limits(), state_limits(), store)
        .unwrap();
    for run in runs {
        assert_eq!(
            serde_json::to_value(host.read_inspection(run, export(), store).unwrap()).unwrap(),
            serde_json::to_value(engine.project(cp.stamp(), run, export()).unwrap()).unwrap()
        );
        assert_eq!(
            host.read_outputs(run, store).unwrap(),
            engine.outputs(run).unwrap()
        );
    }
}

#[tokio::test]
async fn evicted_records_support_fanout_cancellation_demand_and_retained_reuse() {
    let dir = tempfile::tempdir().unwrap();
    let mut store = SqlStore::open(&dir.path().join("owner.db"));
    let mut def = definition();
    def.nodes.get_mut("second").unwrap().activation = Activation::OnDemand;
    let mut never = node(source("first"));
    never.activation = Activation::Disabled;
    def.nodes.insert("never".into(), never);
    let graph = Arc::new(registry().compile(def).unwrap());
    let mut host = DurableEngine::create([graph.clone()], limits(), &mut store).unwrap();
    begin_host(&mut host, &mut store, &graph, "a");
    evict(&mut host, &mut store, &["a"]);
    begin_host(&mut host, &mut store, &graph, "b");
    evict(&mut host, &mut store, &["a", "b"]);
    let work = host.apply(capacity(1), &mut store).unwrap();
    assert_eq!(work.len(), 1);
    let a = attempt(&host, &mut store, "a", "first");
    let b = attempt(&host, &mut store, "b", "first");
    evict(&mut host, &mut store, &["a", "b"]);
    let invocation = host.claim("a", "first", a, &mut store).unwrap();
    evict(&mut host, &mut store, &["a", "b"]);
    host.apply(
        Event::Pause {
            run: "b".into(),
            paused: true,
        },
        &mut store,
    )
    .unwrap();
    evict(&mut host, &mut store, &["a", "b"]);
    host.apply(
        Event::Cancel {
            run: "a".into(),
            node: None,
        },
        &mut store,
    )
    .unwrap();
    evict(&mut host, &mut store, &["a", "b"]);
    host.apply(
        Event::Settle {
            execution: work[0].execution,
            outcome: invocation.execute(evidence_limits()).await.outcome,
        },
        &mut store,
    )
    .unwrap();
    evict(&mut host, &mut store, &["a", "b"]);
    assert!(host.adopt("a", "first", a, &mut store).is_err());
    host.adopt("b", "first", b, &mut store).unwrap();
    evict(&mut host, &mut store, &["a", "b"]);
    host.apply(
        Event::Pause {
            run: "b".into(),
            paused: false,
        },
        &mut store,
    )
    .unwrap();
    evict(&mut host, &mut store, &["a", "b"]);
    host.apply(demand("b", "second"), &mut store).unwrap();
    evict(&mut host, &mut store, &["a", "b"]);
    let second = host.apply(capacity(1), &mut store).unwrap().remove(0);
    let b2 = attempt(&host, &mut store, "b", "second");
    evict(&mut host, &mut store, &["a", "b"]);
    let invocation = host.claim("b", "second", b2, &mut store).unwrap();
    evict(&mut host, &mut store, &["a", "b"]);
    host.apply(
        Event::Settle {
            execution: second.execution,
            outcome: invocation.execute(evidence_limits()).await.outcome,
        },
        &mut store,
    )
    .unwrap();
    evict(&mut host, &mut store, &["a", "b"]);
    host.adopt("b", "second", b2, &mut store).unwrap();
    evict(&mut host, &mut store, &["a", "b"]);
    assert_eq!(
        host.read_outputs("b", &mut store).unwrap(),
        Some(values(42))
    );
    begin_host(&mut host, &mut store, &graph, "c");
    evict(&mut host, &mut store, &["a", "b", "c"]);
    assert!(host.apply(capacity(1), &mut store).unwrap().is_empty());
    let c = host.read_inspection("c", export(), &mut store).unwrap();
    assert_eq!(c.attempts["first"][0].acquisition, Acquisition::Retained);
    assert_eq!(c.nodes.len(), 3);
    assert_eq!(c.nodes["never"], Disposition::Disabled);
    evict(&mut host, &mut store, &["a", "b", "c"]);
    replay_matches(&host, &mut store, graph.clone(), &["a", "b", "c"]);
    let cp = Checkpoint::decode(&store.bytes(), limits().checkpoint).unwrap();
    let mut recovered = DurableEngine::recover(cp, [graph], limits(), &mut store).unwrap();
    assert_eq!(
        recovered.read_outputs("b", &mut store).unwrap(),
        Some(values(42))
    );
    evict(&mut recovered, &mut store, &["a", "b", "c"]);
    assert_eq!(store.publications().len(), 2);
}

#[test]
fn eviction_keeps_retry_identity_and_queued_and_running_recovery_semantics() {
    for dispatched in [false, true] {
        let dir = tempfile::tempdir().unwrap();
        let mut store = SqlStore::open(&dir.path().join("owner.db"));
        let graph = Arc::new(registry().compile(definition()).unwrap());
        let mut host = DurableEngine::create([graph.clone()], limits(), &mut store).unwrap();
        begin_host(&mut host, &mut store, &graph, "a");
        let work = host.apply(capacity(1), &mut store).unwrap().remove(0);
        let first = attempt(&host, &mut store, "a", "first");
        evict(&mut host, &mut store, &["a"]);
        if dispatched {
            drop(host.claim("a", "first", first, &mut store).unwrap());
        }
        evict(&mut host, &mut store, &["a"]);
        host.apply(Event::Recover, &mut store).unwrap();
        evict(&mut host, &mut store, &["a"]);
        let view = host.read_inspection("a", export(), &mut store).unwrap();
        assert!(matches!(
            view.attempts["first"][0].state,
            AttemptState::Failed(_) | AttemptState::Unknown
        ));
        let stamp = host.stamp().clone();
        host.apply(Event::Recover, &mut store).unwrap();
        assert_eq!(host.stamp(), &stamp);
        host.apply(
            Event::Retry {
                run: "a".into(),
                node: "first".into(),
            },
            &mut store,
        )
        .unwrap();
        evict(&mut host, &mut store, &["a"]);
        host.apply(
            Event::Pause {
                run: "a".into(),
                paused: false,
            },
            &mut store,
        )
        .unwrap();
        evict(&mut host, &mut store, &["a"]);
        let next = host.apply(capacity(1), &mut store).unwrap().remove(0);
        assert_ne!(next.execution, work.execution);
        assert_ne!(attempt(&host, &mut store, "a", "first"), first);
        evict(&mut host, &mut store, &["a"]);
        replay_matches(&host, &mut store, graph, &["a"]);
    }
}

#[test]
fn failed_cold_reads_and_rejected_writes_preserve_the_evicted_host() {
    for failure in ["read", "commit"] {
        let dir = tempfile::tempdir().unwrap();
        let mut store = SqlStore::open(&dir.path().join("owner.db"));
        let (mut host, first, _) = super::record_access::available_host(&mut store);
        evict(&mut host, &mut store, &["a", "unrelated"]);
        let before = store.bytes();
        let stamp = host.stamp().clone();
        let usage = host.resident_usage().unwrap();
        store.fail_before_commit = failure == "commit";
        let mut observed = Observed {
            store: &mut store,
            reads: Vec::new(),
            commits: 0,
            fail: (failure == "read").then_some(RecordKey::Run("unrelated".into())),
        };
        // The unrelated cold run is needed by post-event capacity reservation.
        assert!(host.adopt("a", "first", first, &mut observed).is_err());
        assert_eq!(host.stamp(), &stamp);
        assert_eq!(host.resident_usage().unwrap(), usage);
        assert_eq!(observed.store.bytes(), before);
        assert!(observed.store.publications().is_empty());
        observed.fail = None;
        observed.store.fail_before_commit = false;
        host.adopt("a", "first", first, &mut observed).unwrap();
        assert_eq!(observed.store.publications().len(), 1);
    }
}

#[test]
fn primary_eviction_drops_ownership_and_keeps_identity_counts() {
    let mut rows = crate::ai::graph::records::Records::default();
    rows.insert("row", String::from("retained source payload"));
    let weak = Arc::downgrade(&rows.get_shared("row").unwrap());
    rows.evict();
    assert!(weak.upgrade().is_none());
    assert_eq!(rows.len(), 1);
    assert_eq!(rows.resident_len(), 0);
    assert!(rows.contains_key("row"));
    assert!(serde_json::to_vec(&rows).is_err());
}

#[test]
fn legacy_empty_snapshots_repack_without_an_empty_archive_or_lost_history() {
    use crate::ai::graph::{
        checkpoint_format::{Payload, SavedState},
        checkpoint_legacy::InlineState,
    };
    for format in [2, 3] {
        let dir = tempfile::tempdir().unwrap();
        let mut store = SqlStore::open(&dir.path().join("owner.db"));
        let graph = Arc::new(registry().compile(definition()).unwrap());
        let mut engine = Engine::new([graph.clone()]).unwrap();
        begin(&mut engine, &graph, "a", 40);
        engine.apply(Event::Recover).unwrap();
        let original = Checkpoint::capture(
            &engine,
            &uuid::Uuid::new_v4().to_string(),
            limits().checkpoint,
        )
        .unwrap();
        store
            .commit(CommitRequest {
                expected: None,
                next: &original,
                intent: CommitIntent::Record,
                records: RecordChanges::between(None, &engine.state),
            })
            .unwrap();
        engine.rebase();
        let mut old = original.compacted(&engine, limits().checkpoint).unwrap();
        if format == 2 {
            let Payload::Snapshot(p) = &mut old.envelope.payload else {
                unreachable!()
            };
            p.format = 2;
            p.base.state = SavedState::Inline(InlineState::capture(&engine.state));
            old.envelope.checksum =
                crate::ai::graph::compile::digest(&old.envelope.payload).unwrap();
            old = Checkpoint::decode(
                &serde_json::to_vec(&old.envelope).unwrap(),
                limits().checkpoint,
            )
            .unwrap();
        }
        store
            .commit(CommitRequest {
                expected: Some(original.stamp()),
                next: &old,
                intent: CommitIntent::Compact { archive: &original },
                records: RecordChanges::default(),
            })
            .unwrap();
        let mut host = DurableEngine::recover(old, [graph.clone()], limits(), &mut store).unwrap();
        evict(&mut host, &mut store, &["a"]);
        let cp = Checkpoint::decode(&store.bytes(), limits().checkpoint).unwrap();
        assert_eq!(cp.archive_parent(), Some(original.stamp()));
        assert_eq!(
            store
                .read_archive(original.stamp(), limits().checkpoint.bytes)
                .unwrap(),
            original.bytes()
        );
        replay_matches(&host, &mut store, graph, &["a"]);
    }
}

#[test]
fn forged_commitments_fail_native_replay_even_with_a_valid_outer_checksum() {
    use crate::ai::graph::checkpoint_format::{Payload, SavedState};
    for mutation in ["digest", "identity", "missing", "duplicate", "order"] {
        let dir = tempfile::tempdir().unwrap();
        let mut store = SqlStore::open(&dir.path().join("owner.db"));
        let (mut host, _, _) = super::record_access::available_host(&mut store);
        evict(&mut host, &mut store, &["a", "unrelated"]);
        let before = store.bytes();
        let mut cp = Checkpoint::decode(&before, limits().checkpoint).unwrap();
        let Payload::Snapshot(p) = &mut cp.envelope.payload else {
            unreachable!()
        };
        let mut state = serde_json::to_value(&p.base.state).unwrap();
        let rows = state["rows"].as_array_mut().unwrap();
        match mutation {
            "digest" => rows[0][1] = json!(vec![0; 32]),
            "identity" => rows[0][0] = json!({"Run":"forged"}),
            "missing" => {
                rows.pop();
            }
            "duplicate" => rows.push(rows[0].clone()),
            _ => rows.reverse(),
        }
        p.base.state = SavedState::Commitments(serde_json::from_value(state).unwrap());
        cp.envelope.checksum = crate::ai::graph::compile::digest(&cp.envelope.payload).unwrap();
        let forged = Checkpoint::decode(
            &serde_json::to_vec(&cp.envelope).unwrap(),
            limits().checkpoint,
        )
        .unwrap();
        let graph = Arc::new(registry().compile(definition()).unwrap());
        assert_eq!(
            DurableEngine::recover(forged, [graph], limits(), &mut store)
                .err()
                .unwrap()
                .code,
            "snapshot_mismatch",
            "{mutation}"
        );
        assert_eq!(store.bytes(), before);
    }
}

#[test]
fn eviction_rejection_retains_payloads_and_uncertain_archive_commit_requires_reload() {
    for uncertain in [false, true] {
        let dir = tempfile::tempdir().unwrap();
        let mut store = SqlStore::open(&dir.path().join("owner.db"));
        let (mut host, _, _) = super::record_access::available_host(&mut store);
        let before = store.bytes();
        let usage = host.resident_usage().unwrap();
        store.fail_before_commit = !uncertain;
        store.fail_after_commit = uncertain;
        assert!(host.evict_records(&mut store).is_err());
        assert_eq!(host.poisoned(), uncertain);
        if uncertain {
            assert_eq!(
                host.read_outputs("a", &mut store).unwrap_err().code,
                "reload_required"
            );
            assert_ne!(store.bytes(), before);
        } else {
            assert_eq!(host.resident_usage().unwrap(), usage);
            assert_eq!(store.bytes(), before);
        }
        store.fail_before_commit = false;
        store.fail_after_commit = false;
        let graph = Arc::new(registry().compile(definition()).unwrap());
        let cp = Checkpoint::decode(&store.bytes(), limits().checkpoint).unwrap();
        let mut recovered = DurableEngine::recover(cp, [graph], limits(), &mut store).unwrap();
        evict(&mut recovered, &mut store, &["a", "unrelated"]);
    }
}
