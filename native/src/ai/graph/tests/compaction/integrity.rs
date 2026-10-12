use super::*;

fn compacted(store: &mut SqlStore) -> Arc<Executable> {
    let graph = Arc::new(registry().compile(definition()).unwrap());
    let mut host = DurableEngine::create([graph.clone()], limits(), store).unwrap();
    start(&mut host, store, &graph);
    host.apply(capacity(1), store).unwrap();
    let first = id(&host, "first");
    let _invocation = host.claim("run", "first", first, store).unwrap();
    host.compact(store).unwrap();
    graph
}

#[test]
fn forged_native_state_is_rejected_even_with_a_recomputed_integrity_hash() {
    for field in ["next_id", "rows", "capacity", "held"] {
        let dir = tempfile::tempdir().unwrap();
        let mut store = SqlStore::open(&dir.path().join("owner.db"));
        let graph = compacted(&mut store);
        let before = store.bytes();
        let mut cp = checkpoint(&store);
        let super::super::super::checkpoint_format::Payload::Snapshot(p) = &mut cp.envelope.payload
        else {
            panic!("expected snapshot")
        };
        let mut state = serde_json::to_value(&p.base.state).unwrap();
        state[field] = match field {
            "next_id" => json!(999),
            "rows" => json!([]),
            "capacity" => json!({"local":999,"provider":999}),
            _ => json!([["run", "first"]]),
        };
        p.base.state = crate::ai::graph::checkpoint_format::SavedState::Commitments(
            serde_json::from_value(state).unwrap(),
        );
        cp.envelope.checksum = crate::ai::graph::compile::digest(&cp.envelope.payload).unwrap();
        let bytes = serde_json::to_vec(&cp.envelope).unwrap();
        let cp = Checkpoint::decode(&bytes, limits().checkpoint).unwrap();
        assert_eq!(
            DurableEngine::recover(cp, [graph], limits(), &mut store)
                .err()
                .unwrap()
                .code,
            "snapshot_mismatch",
            "{field}"
        );
        assert_eq!(store.bytes(), before);
        assert!(store.publications().is_empty());
    }
}

#[test]
fn missing_corrupt_or_wrong_archive_blocks_recovery_without_modifying_the_root() {
    for mutation in ["missing", "corrupt", "wrong"] {
        let dir = tempfile::tempdir().unwrap();
        let mut store = SqlStore::open(&dir.path().join("owner.db"));
        let graph = compacted(&mut store);
        let before = store.bytes();
        match mutation {
            "missing" => {
                store.conn.execute("DELETE FROM archive", []).unwrap();
            }
            "corrupt" => {
                store
                    .conn
                    .execute("UPDATE archive SET payload=?1", [b"broken".as_slice()])
                    .unwrap();
            }
            _ => {
                let engine = Engine::new([graph.clone()]).unwrap();
                let other = Checkpoint::capture(
                    &engine,
                    &uuid::Uuid::new_v4().to_string(),
                    limits().checkpoint,
                )
                .unwrap();
                store
                    .conn
                    .execute("UPDATE archive SET payload=?1", [other.bytes()])
                    .unwrap();
            }
        }
        assert!(DurableEngine::recover(checkpoint(&store), [graph], limits(), &mut store).is_err());
        assert_eq!(store.bytes(), before);
        assert!(store.publications().is_empty());
    }
}

#[test]
fn history_reads_are_bounded_and_future_snapshot_fields_are_not_ignored() {
    let dir = tempfile::tempdir().unwrap();
    let mut store = SqlStore::open(&dir.path().join("owner.db"));
    let graph = compacted(&mut store);
    for history in [
        HistoryLimits {
            bytes: 1,
            ..history_limits()
        },
        HistoryLimits {
            segments: 0,
            ..history_limits()
        },
        HistoryLimits {
            events: 0,
            ..history_limits()
        },
    ] {
        let before = store.bytes();
        assert!(
            DurableEngine::recover(
                checkpoint(&store),
                [graph.clone()],
                DurableLimits {
                    history,
                    ..limits()
                },
                &mut store
            )
            .is_err()
        );
        assert_eq!(store.bytes(), before);
    }
    let mut value: serde_json::Value = serde_json::from_slice(&store.bytes()).unwrap();
    value["payload"]["base"]["state"]["future_semantics"] = json!(true);
    assert_eq!(
        Checkpoint::decode(&serde_json::to_vec(&value).unwrap(), limits().checkpoint)
            .err()
            .unwrap()
            .code,
        "invalid_checkpoint"
    );
    value["payload"]["format"] = json!(10);
    assert_eq!(
        Checkpoint::decode(&serde_json::to_vec(&value).unwrap(), limits().checkpoint)
            .err()
            .unwrap()
            .code,
        "checkpoint_version"
    );
    assert_eq!(
        checkpoint(&store)
            .replay([graph], state_limits())
            .err()
            .unwrap()
            .code,
        "history_required"
    );
}
