use super::*;
use crate::ai::graph::checkpoint_format::Payload;

fn structured() -> ResponseEvidence {
    ResponseEvidence {
        additional: BTreeMap::from([(
            "response".into(),
            EvidenceValue::ClassifiedJson(json!({
                "usage":{"cost":0.0005,"output_tokens":null},
                "timings":[1,true,null],"empty":{},"code":"nested-marker"
            })),
        )]),
        ..Default::default()
    }
}

#[tokio::test]
async fn structured_success_and_failure_survive_eviction_archive_and_reopen() {
    for outcome in [
        Ok(values(40)),
        Err(unclassified("transport", "response")),
        Ok(BTreeMap::new()),
    ] {
        let mut f = Fixture::new().await;
        f.host
            .record_observations(&f.prefix(), &mut f.store)
            .unwrap();
        f.report.observations.push(structured());
        f.report.outcome = outcome;
        f.host.settle_report(&f.report, &mut f.store).unwrap();
        let expected = f.evidence().unwrap();
        let revision = f.host.stamp().revision;
        assert_eq!(expected.observations.last(), Some(&structured()));
        assert_eq!(
            serde_json::from_slice::<serde_json::Value>(&f.store.bytes()).unwrap()["payload"]["format"],
            7
        );
        let row = f
            .store
            .read_record(
                f.host.stamp(),
                &RecordKey::Execution(f.report.identity.execution),
                limits().checkpoint.bytes,
            )
            .unwrap();
        assert_eq!(
            serde_json::from_slice::<serde_json::Value>(&row).unwrap()["format"],
            4
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
                .contains("nested-marker")
        );
        f.host.evict_records(&mut f.store).unwrap();
        assert_eq!(f.evidence(), Some(expected.clone()));
        let history = f
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
            history
                .execution_evidence(f.report.identity.execution)
                .unwrap(),
            Some(expected.clone())
        );
        f.reopen();
        assert_eq!(f.evidence(), Some(expected));
    }
}

#[tokio::test]
async fn structured_evidence_rejects_old_envelopes_even_after_compaction() {
    let mut f = Fixture::new().await;
    f.report.observations.push(structured());
    f.host.settle_report(&f.report, &mut f.store).unwrap();
    for format in [5, 6] {
        let mut cp = f.checkpoint();
        let Payload::Evidence(payload) = &mut cp.envelope.payload else {
            unreachable!()
        };
        payload.format = format;
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
    }
    f.host.evict_records(&mut f.store).unwrap();
    let mut cp = f.checkpoint();
    let Payload::Evidence(payload) = &mut cp.envelope.payload else {
        unreachable!()
    };
    assert!(payload.events.is_empty());
    assert_eq!(payload.format, 7);
    payload.format = 6;
    cp.envelope.checksum = crate::ai::graph::compile::digest(&cp.envelope.payload).unwrap();
    let cp = Checkpoint::decode(
        &serde_json::to_vec(&cp.envelope).unwrap(),
        limits().checkpoint,
    )
    .unwrap();
    assert_eq!(
        cp.historical_inspection(
            cp.stamp().revision,
            HistoricalLimits {
                history: history_limits(),
                state: state_limits()
            },
            &mut f.store
        )
        .err()
        .unwrap()
        .code,
        "checkpoint_version"
    );
}

#[tokio::test]
async fn structured_observations_are_bounded_and_rejected_settlement_is_atomic() {
    let mut f = Fixture::new().await;
    let before = f.host.stamp().clone();
    f.report.observations.push(ResponseEvidence {
        additional: BTreeMap::from([(
            "large".into(),
            EvidenceValue::ClassifiedJson(json!({"code":"x".repeat(10_000)})),
        )]),
        ..Default::default()
    });
    assert!(f.host.settle_report(&f.report, &mut f.store).is_err());
    assert_eq!(f.host.stamp(), &before);
    assert!(f.evidence().is_none());
    f.report.observations.pop();
    f.report.observations.push(structured());
    f.host.settle_report(&f.report, &mut f.store).unwrap();
    assert!(f.evidence().unwrap().complete);
}
