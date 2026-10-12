use super::*;
use crate::ai::graph::checkpoint_format::{JournalPayload, Payload, SavedState};

fn reencode(checkpoint: &mut Checkpoint) -> Checkpoint {
    checkpoint.envelope.checksum =
        crate::ai::graph::compile::digest(&checkpoint.envelope.payload).unwrap();
    Checkpoint::decode(
        &serde_json::to_vec(&checkpoint.envelope).unwrap(),
        limits().checkpoint,
    )
    .unwrap()
}

#[tokio::test]
async fn dispatch_reserves_the_format_upgrade_before_exposing_an_invocation() {
    let f = Fixture::new().await;
    let settlement_bytes = serde_json::to_vec(&Event::SettleObserved(f.report.clone()))
        .unwrap()
        .len();
    let adoption_bytes = serde_json::to_vec(&Event::Adopt {
        run: "a".into(),
        node: "first".into(),
        attempt: f.attempt,
    })
    .unwrap()
    .len();
    let recovery_bytes = serde_json::to_vec(&Event::Recover).unwrap().len();
    let old_budget = f.store.bytes().len() + settlement_bytes + adoption_bytes + recovery_bytes + 3;
    let header = f.checkpoint().evidence_header_reservation();
    assert!(header > 0);
    for extra in [header - 1, header] {
        let dir = tempfile::tempdir().unwrap();
        let mut store = SqlStore::open(&dir.path().join("owner.db"));
        let mut bound = limits();
        bound.settlement_event_bytes = settlement_bytes;
        bound.checkpoint.bytes = old_budget + extra;
        let mut host = DurableEngine::create([f.graph.clone()], bound, &mut store).unwrap();
        begin_host(&mut host, &mut store, &f.graph, "a");
        host.apply(capacity(1), &mut store).unwrap();
        let attempt = host.inspect("a").unwrap().attempts["first"][0].id;
        let invocation = host.claim("a", "first", attempt, &mut store);
        if extra < header {
            assert_eq!(invocation.err().unwrap().code, "checkpoint_byte_limit");
            assert_eq!(
                host.inspect("a").unwrap().nodes["first"],
                Disposition::Prepared
            );
        } else {
            let report = invocation.unwrap().execute(evidence_limits()).await;
            host.settle_report(&report, &mut store).unwrap();
            host.adopt("a", "first", attempt, &mut store).unwrap();
            assert_eq!(store.publications().len(), 1);
        }
    }
}

#[tokio::test]
async fn every_previous_checkpoint_representation_can_continue_with_evidence() {
    for (format, mode) in (1..=4).flat_map(|format| (0..=2).map(move |mode| (format, mode))) {
        let dir = tempfile::tempdir().unwrap();
        let mut store = SqlStore::open(&dir.path().join("owner.db"));
        let graph = graph();
        let mut host = DurableEngine::create([graph.clone()], limits(), &mut store).unwrap();
        begin_host(&mut host, &mut store, &graph, "a");
        host.apply(
            Event::Pause {
                run: "a".into(),
                paused: true,
            },
            &mut store,
        )
        .unwrap();
        let original = Checkpoint::decode(&store.bytes(), limits().checkpoint).unwrap();
        if format != 1 {
            let mut engine = original.replay([graph.clone()], state_limits()).unwrap();
            engine.rebase();
            let mut old = original.compacted(&engine, limits().checkpoint).unwrap();
            let Payload::Snapshot(payload) = &mut old.envelope.payload else {
                unreachable!()
            };
            payload.format = format;
            payload.base.state = match format {
                2 => SavedState::Inline(crate::ai::graph::checkpoint_legacy::InlineState::capture(
                    &engine.state,
                )),
                3 => SavedState::Records(Box::new(engine.state.clone())),
                _ => {
                    let evidence = crate::ai::graph::record_evidence::RecordEvidence::from_state(
                        &engine.state,
                        original.stamp(),
                        limits().checkpoint.bytes,
                    )
                    .unwrap();
                    SavedState::Commitments(
                        crate::ai::graph::checkpoint_records::RecordState::capture(
                            &engine.state,
                            &evidence,
                        ),
                    )
                }
            };
            let old = reencode(&mut old);
            store
                .commit(CommitRequest {
                    expected: Some(original.stamp()),
                    next: &old,
                    intent: CommitIntent::Compact { archive: &original },
                    records: RecordChanges::default(),
                })
                .unwrap();
        }
        let old = Checkpoint::decode(&store.bytes(), limits().checkpoint).unwrap();
        assert_eq!(
            serde_json::from_slice::<serde_json::Value>(old.bytes()).unwrap()["payload"]["format"],
            format
        );
        host = DurableEngine::recover(old, [graph.clone()], limits(), &mut store).unwrap();
        host.apply(
            Event::Pause {
                run: "a".into(),
                paused: false,
            },
            &mut store,
        )
        .unwrap();
        host.apply(capacity(1), &mut store).unwrap();
        let attempt = host.inspect("a").unwrap().attempts["first"][0].id;
        let mut report = host
            .claim("a", "first", attempt, &mut store)
            .unwrap()
            .execute(evidence_limits())
            .await;
        if mode == 1 {
            report.provisional = Some(ProvisionalCapture {
                session: uuid::Uuid::new_v4().to_string(),
                sequence: 1,
                text: "retained provisional source".into(),
                failure: None,
            });
        }
        if mode == 2 {
            report.observations.push(ResponseEvidence {
                additional: BTreeMap::from([(
                    "nested".into(),
                    EvidenceValue::ClassifiedJson(json!({"usage": [1, null, 0.5]})),
                )]),
                ..Default::default()
            });
        }
        host.settle_report(&report, &mut store).unwrap();
        assert_eq!(
            serde_json::from_slice::<serde_json::Value>(&store.bytes()).unwrap()["payload"]["format"],
            match mode {
                1 => 6,
                2 => 7,
                _ => 5,
            }
        );
        let before = host
            .read_execution_evidence(report.identity.execution, &mut store)
            .unwrap();
        host.evict_records(&mut store).unwrap();
        let saved = Checkpoint::decode(&store.bytes(), limits().checkpoint).unwrap();
        host = DurableEngine::recover(saved, [graph], limits(), &mut store).unwrap();
        assert_eq!(
            host.read_execution_evidence(report.identity.execution, &mut store)
                .unwrap(),
            before
        );
        if format != 1 {
            assert_eq!(
                store
                    .read_archive(original.stamp(), limits().checkpoint.bytes)
                    .unwrap(),
                original.bytes()
            );
        }
    }
}

#[tokio::test]
async fn evidence_requires_new_checkpoint_and_record_versions_and_verified_reads() {
    let mut f = Fixture::new().await;
    let key = RecordKey::Execution(f.report.identity.execution);
    let before = f
        .store
        .read_record(f.host.stamp(), &key, limits().checkpoint.bytes)
        .unwrap();
    let row: serde_json::Value = serde_json::from_slice(&before).unwrap();
    assert_eq!(row["format"], 1);
    assert!(row["value"].get("evidence").is_none());
    let mut invalid_legacy = row["value"].clone();
    invalid_legacy["evidence"] = serde_json::Value::Null;
    assert!(serde_json::from_value::<crate::ai::graph::state::Execution>(invalid_legacy).is_err());
    f.host.settle_report(&f.report, &mut f.store).unwrap();
    let bytes = f
        .store
        .read_record(f.host.stamp(), &key, limits().checkpoint.bytes)
        .unwrap();
    let mut row: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(row["format"], 2);
    assert_eq!(row["value"]["evidence"]["complete"], true);

    let mut cp = f.checkpoint();
    let Payload::Evidence(p) = cp.envelope.payload else {
        unreachable!()
    };
    cp.envelope.payload = Payload::Journal(JournalPayload {
        format: 1,
        engine: p.engine,
        artifacts: p.artifacts,
        events: p.events,
    });
    cp.envelope.checksum = crate::ai::graph::compile::digest(&cp.envelope.payload).unwrap();
    assert_eq!(
        Checkpoint::decode(
            &serde_json::to_vec(&cp.envelope).unwrap(),
            limits().checkpoint
        )
        .err()
        .unwrap()
        .code,
        "checkpoint_version"
    );

    row["value"]["evidence"]["observations"][0]["requested_model"] = json!("tampered");
    f.store
        .conn
        .execute(
            "UPDATE graph_record SET payload=?1 WHERE key=?2",
            rusqlite::params![
                serde_json::to_vec(&row).unwrap(),
                serde_json::to_string(&key).unwrap()
            ],
        )
        .unwrap();
    assert_eq!(
        f.host
            .read_execution_evidence(f.report.identity.execution, &mut f.store)
            .unwrap_err()
            .code,
        "record_mismatch"
    );
}

#[tokio::test]
async fn capture_failure_is_retained_and_cannot_be_cleared_by_later_reports() {
    let mut f = Fixture::new().await;
    let mut prefix = f.prefix();
    prefix.evidence_failure = Some(unclassified("evidence_limit", "observations"));
    f.host.record_observations(&prefix, &mut f.store).unwrap();
    assert_eq!(
        f.host
            .settle_report(&f.report, &mut f.store)
            .unwrap_err()
            .code,
        "evidence_conflict"
    );
    f.report.observations = prefix.observations;
    f.report.evidence_failure = prefix.evidence_failure;
    // Even a forged successful outcome cannot bypass the failed capture.
    f.host.settle_report(&f.report, &mut f.store).unwrap();
    assert_eq!(
        f.host.inspect("a").unwrap().nodes["first"],
        Disposition::Failed
    );
    assert_eq!(
        f.evidence().unwrap().evidence_failure,
        f.report.evidence_failure
    );
    f.reopen();
    assert!(f.evidence().unwrap().complete);
    assert_eq!(
        f.host.inspect("a").unwrap().nodes["first"],
        Disposition::Failed
    );
}
