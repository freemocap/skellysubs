use super::{
    durable_store::SqlStore,
    record_access::{Observed, available_host, limits},
    *,
};
use crate::ai::graph::{
    record_access::{RecordAccess, StoredAccess},
    record_evidence::RecordEvidence,
};

#[test]
fn stored_reader_validates_records_after_native_state_and_checkpoint_are_dropped() {
    let dir = tempfile::tempdir().unwrap();
    let mut store = SqlStore::open(&dir.path().join("owner.db"));
    let (evidence, attempt, execution) = {
        let (host, attempt, execution) = available_host(&mut store);
        let cp = Checkpoint::decode(&store.bytes(), limits().checkpoint).unwrap();
        let graph = Arc::new(registry().compile(definition()).unwrap());
        let (engine, _) = cp
            .replay_history([graph], history_limits(), state_limits(), &mut store)
            .unwrap();
        (
            RecordEvidence::from_state(&engine.state, host.stamp(), limits().checkpoint.bytes)
                .unwrap(),
            attempt,
            execution,
        )
    };
    let mut observed = Observed {
        store: &mut store,
        reads: Vec::new(),
        fail: None,
        commits: 0,
    };
    let mut records = StoredAccess {
        evidence: &evidence,
        max_bytes: limits().checkpoint.bytes,
        store: &mut observed,
    };
    assert_eq!(records.run("a").unwrap().inputs, values(40));
    assert_eq!(
        records.attempt(attempt).unwrap().attempt.state,
        AttemptState::Available
    );
    assert_eq!(
        records.execution(execution).unwrap().outcome,
        Some(Ok(values(41)))
    );
    assert_eq!(records.run("missing").unwrap_err().code, "unknown_run");
    assert_eq!(observed.reads.len(), 3);
    assert_eq!(observed.commits, 0);
}

#[test]
fn evidence_tracks_only_acknowledged_commits_and_compaction_revisions() {
    let dir = tempfile::tempdir().unwrap();
    let mut store = SqlStore::open(&dir.path().join("owner.db"));
    let (mut host, _, _) = available_host(&mut store);
    let export = ExportLimits {
        bytes: 1_000_000,
        attempts: 1000,
    };
    let before =
        serde_json::to_value(host.read_inspection("a", export, &mut store).unwrap()).unwrap();
    store.fail_before_commit = true;
    assert!(
        host.apply(
            Event::Pause {
                run: "a".into(),
                paused: true
            },
            &mut store
        )
        .is_err()
    );
    assert_eq!(
        serde_json::to_value(host.read_inspection("a", export, &mut store).unwrap()).unwrap(),
        before
    );
    store.fail_before_commit = false;
    host.apply(
        Event::Pause {
            run: "a".into(),
            paused: true,
        },
        &mut store,
    )
    .unwrap();
    assert!(
        host.read_inspection("a", export, &mut store)
            .unwrap()
            .paused
    );
    let before_compact =
        serde_json::to_value(host.read_inspection("a", export, &mut store).unwrap()).unwrap();
    assert!(host.compact(&mut store).unwrap());
    assert_eq!(
        serde_json::to_value(host.read_inspection("a", export, &mut store).unwrap()).unwrap(),
        before_compact
    );
    store.fail_after_commit = true;
    assert!(
        host.apply(
            Event::Pause {
                run: "a".into(),
                paused: false
            },
            &mut store
        )
        .is_err()
    );
    assert_eq!(
        host.read_inspection("a", export, &mut store)
            .err()
            .unwrap()
            .code,
        "reload_required"
    );
    store.fail_after_commit = false;
    let cp = Checkpoint::decode(&store.bytes(), limits().checkpoint).unwrap();
    let graph = Arc::new(registry().compile(definition()).unwrap());
    let host = DurableEngine::recover(cp, [graph], limits(), &mut store).unwrap();
    assert!(
        host.read_inspection("a", export, &mut store)
            .unwrap()
            .paused
    );
}
