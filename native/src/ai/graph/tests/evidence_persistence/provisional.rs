use super::*;

fn capture(sequence: u64, text: &str) -> ProvisionalCapture {
    ProvisionalCapture {
        session: "938c2c23-76a5-4c61-98bf-a6f0ccf1a97e".into(),
        sequence,
        text: text.into(),
        failure: None,
    }
}

#[tokio::test]
async fn corrected_text_handoff_is_atomic_and_survives_history_eviction_and_restart() {
    let mut f = Fixture::new().await;
    let mut prefix = f.prefix();
    prefix.provisional = Some(capture(1, "provisional 海"));
    f.host.record_observations(&prefix, &mut f.store).unwrap();
    f.report.provisional = Some(capture(2, "corrected é"));
    let before = f.evidence();
    f.store.fail_before_commit = true;
    assert!(f.host.settle_report(&f.report, &mut f.store).is_err());
    assert_eq!(f.evidence(), before);
    assert_eq!(
        f.host.inspect("a").unwrap().nodes["first"],
        Disposition::Running
    );
    f.store.fail_before_commit = false;
    f.host.settle_report(&f.report, &mut f.store).unwrap();
    let saved = f.evidence().unwrap();
    assert!(saved.complete);
    assert_eq!(saved.provisional, f.report.provisional);
    assert!(
        f.store.publications().is_empty(),
        "settlement is not adoption"
    );
    let revision = f.host.stamp().revision;
    f.host.evict_records(&mut f.store).unwrap();
    assert_eq!(f.evidence(), Some(saved.clone()));
    let historical = f
        .checkpoint()
        .historical_inspection(
            revision,
            HistoricalLimits {
                history: history_limits(),
                state: state_limits(),
            },
            &mut f.store,
        )
        .unwrap();
    assert_eq!(
        historical
            .execution_evidence(f.report.identity.execution)
            .unwrap(),
        Some(saved.clone())
    );
    f.reopen();
    assert_eq!(f.evidence(), Some(saved));
    f.host
        .apply(
            Event::Pause {
                run: "a".into(),
                paused: false,
            },
            &mut f.store,
        )
        .unwrap();
    f.host.adopt("a", "first", f.attempt, &mut f.store).unwrap();
    assert_eq!(
        f.store.publications(),
        vec![serde_json::to_string(&values(40)).unwrap()]
    );
}

#[tokio::test]
async fn committed_watermark_rejects_rewrites_stale_sessions_and_erasure() {
    let mut f = Fixture::new().await;
    let mut prefix = f.prefix();
    prefix.provisional = Some(capture(2, "retained"));
    f.host.record_observations(&prefix, &mut f.store).unwrap();
    let stamp = f.host.stamp().clone();
    let before = f.evidence();
    for replacement in [
        None,
        Some(capture(1, "retained")),
        Some(capture(2, "rewritten")),
        Some(ProvisionalCapture {
            session: uuid::Uuid::new_v4().to_string(),
            ..capture(3, "other")
        }),
        Some(ProvisionalCapture {
            session: "invalid".into(),
            ..capture(3, "other")
        }),
    ] {
        let mut conflict = prefix.clone();
        conflict.provisional = replacement;
        assert_eq!(
            f.host
                .record_observations(&conflict, &mut f.store)
                .unwrap_err()
                .code,
            "provisional_conflict"
        );
        assert_eq!(f.host.stamp(), &stamp);
        assert_eq!(f.evidence(), before);
    }
    f.host.record_observations(&prefix, &mut f.store).unwrap();
    assert_eq!(
        f.evidence(),
        before,
        "an identical watermark is a repeatable fact"
    );
    let mut excessive = prefix.clone();
    excessive.provisional = Some(capture(3, &"x".repeat(limits().settlement_event_bytes)));
    assert_eq!(
        f.host
            .record_observations(&excessive, &mut f.store)
            .unwrap_err()
            .code,
        "settlement_limit"
    );
    assert_eq!(f.evidence(), before);
}

#[tokio::test]
async fn capture_failure_cannot_be_cleared_or_forged_into_success() {
    let mut f = Fixture::new().await;
    let mut prefix = f.prefix();
    let mut stopped = capture(1, "last accepted");
    stopped.failure = Some(unclassified("provisional_limit", "provisional"));
    prefix.provisional = Some(stopped.clone());
    f.host.record_observations(&prefix, &mut f.store).unwrap();
    f.report.provisional = Some(capture(1, "last accepted"));
    assert!(f.host.settle_report(&f.report, &mut f.store).is_err());
    f.report.provisional = Some(stopped);
    assert!(f.report.outcome.is_ok());
    f.host.settle_report(&f.report, &mut f.store).unwrap();
    assert_eq!(
        f.host.inspect("a").unwrap().nodes["first"],
        Disposition::Failed
    );
    assert!(f.evidence().unwrap().complete);
    assert!(f.host.adopt("a", "first", f.attempt, &mut f.store).is_err());
}

#[tokio::test]
async fn cancelled_consumer_and_unknown_producer_do_not_regain_authority_from_late_text() {
    let mut f = Fixture::new().await;
    begin_host(&mut f.host, &mut f.store, &f.graph, "b");
    f.host.apply(capacity(1), &mut f.store).unwrap();
    f.host
        .apply(
            Event::Cancel {
                run: "a".into(),
                node: None,
            },
            &mut f.store,
        )
        .unwrap();
    let mut prefix = f.prefix();
    prefix.provisional = Some(capture(1, "late for a, live for b"));
    f.host.record_observations(&prefix, &mut f.store).unwrap();
    assert_eq!(
        f.host.inspect("a").unwrap().nodes["first"],
        Disposition::Cancelled
    );
    assert_eq!(
        f.host.inspect("b").unwrap().nodes["first"],
        Disposition::Running
    );
    f.reopen();
    prefix.provisional = Some(capture(2, "late after recovery"));
    f.host.record_observations(&prefix, &mut f.store).unwrap();
    assert_eq!(
        f.host.inspect("b").unwrap().nodes["first"],
        Disposition::Unknown
    );
    f.report.provisional = prefix.provisional.clone();
    assert!(f.host.settle_report(&f.report, &mut f.store).is_err());
    assert_eq!(f.evidence().unwrap().provisional, prefix.provisional);
    assert!(f.store.publications().is_empty());
}

#[tokio::test]
async fn lost_ack_recovers_only_the_committed_capture_and_never_reissues_work() {
    let mut f = Fixture::new().await;
    let mut prefix = f.prefix();
    prefix.provisional = Some(capture(1, "acknowledged by storage"));
    f.store.fail_after_commit = true;
    assert!(f.host.record_observations(&prefix, &mut f.store).is_err());
    assert!(f.host.poisoned());
    assert!(
        f.host
            .read_execution_evidence(f.report.identity.execution, &mut f.store)
            .is_err()
    );
    f.reopen();
    assert_eq!(f.evidence().unwrap().provisional, prefix.provisional);
    assert!(!f.evidence().unwrap().complete);
    assert!(f.host.apply(capacity(1), &mut f.store).unwrap().is_empty());
}

#[tokio::test]
async fn preview_encoding_is_versioned_and_cannot_leak_into_the_public_projection() {
    let mut f = Fixture::new().await;
    f.host
        .record_observations(&f.prefix(), &mut f.store)
        .unwrap();
    assert_eq!(
        serde_json::from_slice::<serde_json::Value>(&f.store.bytes()).unwrap()["payload"]["format"],
        5
    );
    f.report.provisional = Some(capture(1, "private-source-marker"));
    f.host.settle_report(&f.report, &mut f.store).unwrap();
    let key = RecordKey::Execution(f.report.identity.execution);
    let row: serde_json::Value = serde_json::from_slice(
        &f.store
            .read_record(f.host.stamp(), &key, limits().checkpoint.bytes)
            .unwrap(),
    )
    .unwrap();
    assert_eq!(row["format"], 3);
    let mut explicit_null = serde_json::to_value(&f.report).unwrap();
    explicit_null["provisional"] = serde_json::Value::Null;
    assert!(serde_json::from_value::<InvocationReport>(explicit_null).is_err());
    let mut cp = f.checkpoint();
    let crate::ai::graph::checkpoint_format::Payload::Evidence(payload) = &mut cp.envelope.payload
    else {
        unreachable!()
    };
    assert_eq!(payload.format, 6);
    payload.format = 5;
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
    let view = f
        .host
        .inspection_snapshot(
            "a",
            ExportLimits {
                bytes: 100_000,
                attempts: 100,
            },
        )
        .unwrap();
    assert!(
        !serde_json::to_string(&view)
            .unwrap()
            .contains("private-source-marker")
    );
    f.host.evict_records(&mut f.store).unwrap();
    assert_eq!(
        serde_json::from_slice::<serde_json::Value>(&f.store.bytes()).unwrap()["payload"]["format"],
        6
    );
    let mut downgraded = f.checkpoint();
    let crate::ai::graph::checkpoint_format::Payload::Evidence(p) =
        &mut downgraded.envelope.payload
    else {
        unreachable!()
    };
    assert!(p.events.is_empty());
    p.format = 5;
    downgraded.envelope.checksum =
        crate::ai::graph::compile::digest(&downgraded.envelope.payload).unwrap();
    let downgraded = Checkpoint::decode(
        &serde_json::to_vec(&downgraded.envelope).unwrap(),
        limits().checkpoint,
    )
    .unwrap();
    assert_eq!(
        downgraded
            .historical_inspection(
                downgraded.stamp().revision,
                HistoricalLimits {
                    history: history_limits(),
                    state: state_limits(),
                },
                &mut f.store
            )
            .err()
            .unwrap()
            .code,
        "checkpoint_version"
    );
    f.reopen();
    assert_eq!(f.evidence().unwrap().provisional, f.report.provisional);
}

#[tokio::test]
async fn dispatch_reserves_preview_report_and_version_header_at_the_exact_bound() {
    let mut f = Fixture::new().await;
    f.report.provisional = Some(capture(2, "corrected source"));
    let report_bytes = serde_json::to_vec(&Event::SettleObserved(f.report.clone()))
        .unwrap()
        .len();
    let adoption_bytes = serde_json::to_vec(&Event::Adopt {
        run: "a".into(),
        node: "first".into(),
        attempt: f.attempt,
    })
    .unwrap()
    .len();
    let exact = f.store.bytes().len()
        + report_bytes
        + adoption_bytes
        + serde_json::to_vec(&Event::Recover).unwrap().len()
        + 3
        + f.checkpoint().evidence_header_reservation();
    for bytes in [exact - 1, exact] {
        let dir = tempfile::tempdir().unwrap();
        let mut store = SqlStore::open(&dir.path().join("owner.db"));
        let mut bound = limits();
        bound.settlement_event_bytes = report_bytes;
        bound.checkpoint.bytes = bytes;
        let mut host = DurableEngine::create([f.graph.clone()], bound, &mut store).unwrap();
        begin_host(&mut host, &mut store, &f.graph, "a");
        host.apply(capacity(1), &mut store).unwrap();
        let attempt = host.inspect("a").unwrap().attempts["first"][0].id;
        let claimed = host.claim("a", "first", attempt, &mut store);
        if bytes < exact {
            assert_eq!(claimed.err().unwrap().code, "checkpoint_byte_limit");
        } else {
            let mut report = claimed.unwrap().execute(evidence_limits()).await;
            report.provisional = f.report.provisional.clone();
            host.settle_report(&report, &mut store).unwrap();
            host.adopt("a", "first", attempt, &mut store).unwrap();
            assert_eq!(store.publications().len(), 1);
        }
    }
}
