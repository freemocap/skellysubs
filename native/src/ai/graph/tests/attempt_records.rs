use super::*;
use crate::ai::graph::{
    checkpoint_format::{Payload, SavedState},
    checkpoint_legacy::InlineState,
};
use durable_store::SqlStore;

fn limits() -> CheckpointLimits {
    CheckpointLimits {
        bytes: 2_000_000,
        events: 1000,
    }
}

fn trace(retries: usize) -> (Arc<Executable>, Engine) {
    let mut definition = definition();
    definition.nodes.get_mut("second").unwrap().activation = Activation::OnDemand;
    let graph = Arc::new(registry().compile(definition).unwrap());
    let mut engine = Engine::new([graph.clone()]).unwrap();
    for run in ["a", "b"] {
        begin(&mut engine, &graph, run, 40);
    }
    for turn in 0..=retries {
        if turn > 0 {
            engine
                .apply(Event::Retry {
                    run: "a".into(),
                    node: "first".into(),
                })
                .unwrap();
        }
        let before = engine.state.clone();
        let work = advance(&mut engine, capacity(1)).remove(0);
        for (id, record) in before.attempts.iter() {
            assert!(
                std::ptr::eq(record, &engine.state.attempts[id]),
                "old attempt changed during retry allocation"
            );
        }
        engine
            .apply(Event::Settle {
                execution: work.execution,
                outcome: Err(unclassified("fixture", "provider")),
            })
            .unwrap();
    }
    (graph, engine)
}

fn root(engine: &Engine, store: &mut SqlStore) -> Checkpoint {
    let checkpoint =
        Checkpoint::capture(engine, &uuid::Uuid::new_v4().to_string(), limits()).unwrap();
    store
        .commit(CommitRequest {
            expected: None,
            next: &checkpoint,
            intent: CommitIntent::Record,
            records: RecordChanges::between(None, &engine.state),
        })
        .unwrap();
    checkpoint
}

fn commit_compaction(previous: &Checkpoint, next: &Checkpoint, store: &mut SqlStore) {
    store
        .commit(CommitRequest {
            expected: Some(previous.stamp()),
            next,
            intent: CommitIntent::Compact { archive: previous },
            records: RecordChanges::default(),
        })
        .unwrap();
}

fn rehash(checkpoint: &mut Checkpoint) -> Checkpoint {
    checkpoint.envelope.checksum =
        crate::ai::graph::compile::digest(&checkpoint.envelope.payload).unwrap();
    Checkpoint::decode(&serde_json::to_vec(&checkpoint.envelope).unwrap(), limits()).unwrap()
}

#[test]
fn ownership_index_rebuilds_from_rows_and_isolated_candidates_preserve_it() {
    let (_, engine) = trace(12);
    let attempts = &engine.state.attempts;
    let expected: BTreeMap<_, _> = attempts
        .iter()
        .map(|(id, row)| (*id, row.clone()))
        .collect();
    let bytes = serde_json::to_vec(attempts).unwrap();
    // The existing format-3 row map remains the entire persisted representation.
    assert_eq!(bytes, serde_json::to_vec(&expected).unwrap());
    let decoded: crate::ai::graph::attempts::Attempts = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(&decoded, attempts);
    for run in ["a", "b", "absent"] {
        let ids: Vec<_> = expected
            .iter()
            .filter(|(_, row)| row.run == run)
            .map(|(id, _)| *id)
            .collect();
        assert_eq!(decoded.ids(run), ids);
        assert_eq!(decoded.for_run(run).len(), ids.len());
    }
    let mut candidate = decoded.clone();
    let id = decoded.ids("a")[0];
    candidate.set_state(&id, AttemptState::Cancelled);
    assert_eq!(candidate.ids("a"), decoded.ids("a"));
    assert_ne!(candidate[&id].attempt.state, decoded[&id].attempt.state);
    let mut row = decoded[&id].clone();
    row.attempt.id = AttemptId(999);
    candidate.insert(AttemptId(999), row);
    assert_eq!(candidate.ids("a").len(), decoded.ids("a").len() + 1);
    assert_eq!(candidate.ids("b"), decoded.ids("b"));
    assert!(decoded.get(&AttemptId(999)).is_none());
    let rebuilt: crate::ai::graph::attempts::Attempts =
        serde_json::from_slice(&serde_json::to_vec(&candidate).unwrap()).unwrap();
    assert_eq!(rebuilt, candidate);
}

#[test]
fn run_metadata_is_independent_of_retry_history_and_pages_read_attempt_rows() {
    let (graph, mut engine) = trace(32);
    assert_eq!(engine.state.attempts.len(), 34);
    assert_eq!(engine.state.runs["a"].current.len(), 1);
    assert_eq!(engine.state.runs["b"].current.len(), 1);
    let raw = serde_json::to_value(&engine.state.runs["a"]).unwrap();
    assert!(raw.get("attempts").is_none());
    let before = engine.state.clone();
    engine
        .apply(Event::Pause {
            run: "a".into(),
            paused: true,
        })
        .unwrap();
    for (id, record) in before.attempts.iter() {
        assert!(std::ptr::eq(record, &engine.state.attempts[id]));
    }
    let facts = engine
        .inspect_with_attempts_using(&mut &engine.state, "a", engine.run("a").unwrap(), ())
        .unwrap();
    assert_eq!(facts.artifact, graph.artifact());
    assert_eq!(facts.nodes.len(), 2);
    assert_eq!(engine.inspect("a").unwrap().attempts["first"].len(), 33);

    let dir = tempfile::tempdir().unwrap();
    let mut store = SqlStore::open(&dir.path().join("owner.db"));
    let cp = root(&engine, &mut store);
    let historical = cp
        .historical_inspection(
            engine.revision(),
            HistoricalLimits {
                history: history_limits(),
                state: state_limits(),
            },
            &mut store,
        )
        .unwrap();
    let mut cursor = None;
    let mut ids = Vec::new();
    loop {
        let page = historical
            .page(
                "a",
                cursor.as_ref(),
                ExportLimits {
                    bytes: 100_000,
                    attempts: 3,
                },
            )
            .unwrap();
        assert_eq!(page.attempts.total, "33");
        assert_eq!(page.attempts.offset, ids.len().to_string());
        assert_eq!(page.nodes.len(), 2);
        assert!(page.attempts.records.len() <= 3);
        ids.extend(
            page.attempts
                .records
                .into_iter()
                .map(|r| r.attempt.id.parse::<u64>().unwrap()),
        );
        cursor = page.attempts.next;
        if cursor.is_none() {
            break;
        }
    }
    let expected: Vec<_> = engine
        .state
        .run_attempts("a")
        .map(|r| r.attempt.id.0)
        .collect();
    assert_eq!(ids, expected);
}

#[test]
fn retained_inline_checkpoints_and_normalized_bases_share_one_native_replay() {
    let (graph, engine) = trace(2);
    let dir = tempfile::tempdir().unwrap();
    let mut store = SqlStore::open(&dir.path().join("owner.db"));
    let first = root(&engine, &mut store);
    let mut rebased = engine.clone();
    rebased.rebase();
    let mut inline = first.compacted(&rebased, limits()).unwrap();
    let Payload::Snapshot(payload) = &mut inline.envelope.payload else {
        unreachable!()
    };
    payload.format = 2;
    payload.base.state = SavedState::Inline(InlineState::capture(&engine.state));
    let inline = rehash(&mut inline);
    commit_compaction(&first, &inline, &mut store);
    let (mut restored, _) = inline
        .replay_history(
            [graph.clone()],
            history_limits(),
            state_limits(),
            &mut store,
        )
        .unwrap();
    assert_eq!(restored.state, engine.state);
    restored
        .apply(Event::Pause {
            run: "a".into(),
            paused: true,
        })
        .unwrap();
    let appended = inline.capture_next(&restored, limits()).unwrap();
    store
        .commit(CommitRequest {
            expected: Some(inline.stamp()),
            next: &appended,
            intent: CommitIntent::Record,
            records: RecordChanges::between(None, &restored.state),
        })
        .unwrap();
    restored.rebase();
    let normalized = appended.compacted(&restored, limits()).unwrap();
    assert_eq!(
        serde_json::from_slice::<serde_json::Value>(normalized.bytes()).unwrap()["payload"]["format"],
        json!(3)
    );
    commit_compaction(&appended, &normalized, &mut store);
    let (replayed, _) = normalized
        .replay_history([graph], history_limits(), state_limits(), &mut store)
        .unwrap();
    assert_eq!(replayed.state, restored.state);
    assert_eq!(
        store.read_archive(first.stamp(), limits().bytes).unwrap(),
        first.bytes()
    );
    assert_eq!(
        store
            .read_archive(appended.stamp(), limits().bytes)
            .unwrap(),
        appended.bytes()
    );
    let historical = normalized
        .historical_inspection(
            engine.revision(),
            HistoricalLimits {
                history: history_limits(),
                state: state_limits(),
            },
            &mut store,
        )
        .unwrap();
    assert_eq!(
        historical
            .page(
                "a",
                None,
                ExportLimits {
                    bytes: 100_000,
                    attempts: 100
                }
            )
            .unwrap()
            .attempts
            .total,
        "3"
    );
}

#[test]
fn forged_attempt_ownership_and_current_references_fail_replay_validation() {
    let (graph, engine) = trace(1);
    let dir = tempfile::tempdir().unwrap();
    let mut store = SqlStore::open(&dir.path().join("owner.db"));
    let first = root(&engine, &mut store);
    let mut rebased = engine.clone();
    rebased.rebase();
    let current = engine.state.runs["a"].current["first"];
    let other = engine.state.runs["b"].current["first"];
    // Install an archive, then independently forge each otherwise typed base.
    let valid = first.compacted(&rebased, limits()).unwrap();
    commit_compaction(&first, &valid, &mut store);
    for mutation in [
        "owner",
        "node",
        "identity",
        "current",
        "missing_old",
        "extra",
    ] {
        let mut cp = Checkpoint::decode(valid.bytes(), limits()).unwrap();
        let Payload::Snapshot(payload) = &mut cp.envelope.payload else {
            unreachable!()
        };
        let mut state = serde_json::to_value(&payload.base.state).unwrap();
        let id = current.0.to_string();
        match mutation {
            "owner" => state["attempts"][&id]["run"] = json!("b"),
            "node" => state["attempts"][&id]["node"] = json!("second"),
            "identity" => state["attempts"][&id]["attempt"]["id"] = json!(999),
            "current" => state["runs"]["a"]["current"]["first"] = json!(other.0),
            "missing_old" => {
                state["attempts"].as_object_mut().unwrap().remove("1");
            }
            _ => {
                let row = state["attempts"][&id].clone();
                state["attempts"]["999"] = row;
            }
        }
        payload.base.state = SavedState::Records(serde_json::from_value(state).unwrap());
        let cp = rehash(&mut cp);
        let error = cp
            .replay_history(
                [graph.clone()],
                history_limits(),
                state_limits(),
                &mut store,
            )
            .err()
            .unwrap();
        assert_eq!(error.code, "snapshot_mismatch", "{mutation}");
        assert_eq!(store.bytes(), valid.bytes());
    }
}
