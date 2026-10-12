use super::*;

#[test]
fn cursors_and_output_budgets_fail_explicitly_without_partial_pages() {
    let dir = tempfile::tempdir().unwrap();
    let mut store = SqlStore::open(&dir.path().join("owner.db"));
    let engine = trace(&graph());
    let checkpoint = save(&engine, &mut store);
    let history = checkpoint
        .historical_inspection(checkpoint.stamp().revision, read_limits(), &mut store)
        .unwrap();
    let cursor = history
        .page("a", None, export(1))
        .unwrap()
        .attempts
        .next
        .unwrap();
    for field in [
        "protocol",
        "engine",
        "run",
        "artifact",
        "revision",
        "missing",
        "noncanonical",
        "overflow",
    ] {
        let mut invalid = cursor.clone();
        match field {
            "protocol" => invalid.protocol = 2,
            "engine" => invalid.engine = uuid::Uuid::new_v4().to_string(),
            "run" => invalid.run = "b".into(),
            "artifact" => invalid.artifact = "another".into(),
            "revision" => invalid.revision = "0".into(),
            "missing" => invalid.after_attempt = "99999".into(),
            "noncanonical" => invalid.after_attempt = format!("0{}", cursor.after_attempt),
            _ => invalid.after_attempt = "18446744073709551616".into(),
        }
        assert_eq!(
            history
                .page("a", Some(&invalid), export(1))
                .err()
                .unwrap()
                .code,
            "history_cursor",
            "{field}"
        );
    }
    for limits in [
        export(0),
        ExportLimits {
            bytes: 1,
            ..export(1)
        },
    ] {
        assert_eq!(
            history.page("a", None, limits).err().unwrap().code,
            "inspection_limit"
        );
    }
    assert_eq!(
        checkpoint
            .historical_inspection(checkpoint.stamp().revision + 1, read_limits(), &mut store)
            .err()
            .unwrap()
            .code,
        "inspection_revision"
    );
}

#[test]
fn historical_reader_audits_all_evidence_even_after_the_requested_cut() {
    let dir = tempfile::tempdir().unwrap();
    let mut store = SqlStore::open(&dir.path().join("owner.db"));
    let mut engine = trace(&graph());
    let old = save(&engine, &mut store);
    let mut checkpoint = compact(&mut engine, &old, &mut store);
    let super::super::super::checkpoint_format::Payload::Snapshot(payload) =
        &mut checkpoint.envelope.payload
    else {
        panic!("snapshot")
    };
    let mut state = serde_json::to_value(&payload.base.state).unwrap();
    state["next_id"] = json!(999);
    payload.base.state = crate::ai::graph::checkpoint_format::SavedState::Records(
        serde_json::from_value(state).unwrap(),
    );
    checkpoint.envelope.checksum =
        crate::ai::graph::compile::digest(&checkpoint.envelope.payload).unwrap();
    let forged =
        Checkpoint::decode(&serde_json::to_vec(&checkpoint.envelope).unwrap(), bounds()).unwrap();
    let before = store.bytes();
    assert_eq!(
        forged
            .historical_inspection(1, read_limits(), &mut store)
            .err()
            .unwrap()
            .code,
        "snapshot_mismatch"
    );
    assert_eq!(
        forged
            .run_history("a", Some(2), 1, read_limits(), export(100), &mut store)
            .err()
            .unwrap()
            .code,
        "snapshot_mismatch"
    );
    store.conn.execute("DELETE FROM archive", []).unwrap();
    assert!(
        checkpoint
            .historical_inspection(1, read_limits(), &mut store)
            .is_err()
    );
    assert_eq!(store.bytes(), before);
}

#[test]
fn recorded_structure_is_validated_without_a_registry_or_panicking_on_missing_bindings() {
    let graph = graph();
    for change in ["operation", "type", "constant_type", "cycle"] {
        let mut artifact = graph.artifact().clone();
        match change {
            "operation" => artifact.operations.clear(),
            "type" => artifact.types.clear(),
            "constant_type" => artifact
                .definition
                .nodes
                .get_mut("never")
                .unwrap()
                .inputs
                .insert(
                    "value".into(),
                    Source::Constant {
                        contract: contract("missing"),
                        value: json!(1),
                    },
                )
                .map(|_| ())
                .unwrap(),
            _ => artifact
                .definition
                .nodes
                .get_mut("first")
                .unwrap()
                .after
                .push("second".into()),
        }
        assert!(
            Engine::recorded(
                &BTreeMap::from([(artifact.fingerprint().unwrap(), artifact)]),
                state_limits()
            )
            .is_err(),
            "{change}"
        );
    }
    let mut engine = trace(&graph);
    let events = engine.journal().to_vec();
    engine = Engine::recorded(
        &BTreeMap::from([(graph.identity().into(), graph.artifact().clone())]),
        state_limits(),
    )
    .unwrap();
    for event in events.into_iter().take(3) {
        engine.apply(event).unwrap();
    }
    let execution = engine.inspect("a").unwrap().attempts["first"]
        .last()
        .unwrap()
        .execution;
    let revision = engine.revision();
    assert_eq!(
        engine.claim(execution).err().unwrap().code,
        "executable_required"
    );
    assert_eq!(engine.revision(), revision);
}

#[test]
fn historical_archive_reads_reject_each_budget_without_mutating_storage() {
    let dir = tempfile::tempdir().unwrap();
    let mut store = SqlStore::open(&dir.path().join("owner.db"));
    let mut engine = trace(&graph());
    let old = save(&engine, &mut store);
    let checkpoint = compact(&mut engine, &old, &mut store);
    let before = store.bytes();
    for limits in [
        HistoryLimits {
            bytes: 1,
            ..history_limits()
        },
        HistoryLimits {
            events: 0,
            ..history_limits()
        },
        HistoryLimits {
            segments: 0,
            ..history_limits()
        },
    ] {
        assert!(
            checkpoint
                .historical_inspection(
                    1,
                    HistoricalLimits {
                        history: limits,
                        state: state_limits()
                    },
                    &mut ReadOnly(&mut store)
                )
                .is_err()
        );
        assert_eq!(store.bytes(), before);
    }
    assert!(store.publications().is_empty());
}
