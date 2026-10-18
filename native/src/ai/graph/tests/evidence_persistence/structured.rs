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

#[tokio::test]
async fn diagnostic_export_includes_retained_failure_without_private_graph_values() {
    let mut f = Fixture::new().await;
    let error = crate::model::AppError::new(
        crate::model::ErrorCode::Provider,
        "Speech rejected source-canary; api_key=credential-canary",
    )
    .with_diagnostics(
        json!({"stage":"speech_decode", "request_id":"req-speech-failed",
            "status":502, "path":"audio.data", "expected":"PCM audio", "prompt":"prompt-canary"}),
    );
    f.report
        .observations
        .push(crate::ai::transport::graph_evidence::failure(
            &error,
            "speech-model",
            &["source-canary"],
        ));
    f.report.observations.push(crate::ai::transport::graph_evidence::metadata(
        &json!({"http":{"status":200}, "diagnostics":{
            "stage":"speech_stream", "audio_samples":0,
            "error":{"code":"ELEVENLABS_HTTP_401", "status":502,
                "diagnostics":{"provider":"elevenlabs", "status":401,
                    "request_id":"f97fd76fcb72dfbd4ee6b07cc50867bc",
                    "x_request_id":"5ba1a5767b7f4da4a835e603361a2161",
                    "model":"eleven_v4_turbo", "code":"quota_exceeded",
                    "message":"You have 0 credits remaining, while 7 credits are required for this request"}}}}), &[]));
    f.report.provisional = Some(ProvisionalCapture {
        session: "938c2c23-76a5-4c61-98bf-a6f0ccf1a97e".into(),
        sequence: 1,
        text: "provisional-content-canary".into(),
        failure: None,
    });
    f.report.outcome = Err(unclassified("private-adapter-code", "private-adapter-path"));
    f.host.settle_report(&f.report, &mut f.store).unwrap();
    f.reopen();
    let history = f
        .checkpoint()
        .historical_inspection(
            f.host.stamp().revision,
            HistoricalLimits {
                history: history_limits(),
                state: state_limits(),
            },
            &mut f.store,
        )
        .unwrap();
    let mut exported = Vec::new();
    crate::diagnostics::execution::write_history(&history, &mut exported).unwrap();
    let directory = tempfile::tempdir().unwrap();
    let root = directory.path().join("logs");
    std::fs::create_dir(&root).unwrap();
    std::fs::write(root.join("execution-diagnostics.jsonl"), &exported).unwrap();
    let archive = crate::diagnostics::archive::save(&root, directory.path()).unwrap();
    let mut archive = zip::ZipArchive::new(std::fs::File::open(archive).unwrap()).unwrap();
    let mut text = String::new();
    std::io::Read::read_to_string(
        &mut archive.by_name("logs/execution-diagnostics.jsonl").unwrap(),
        &mut text,
    )
    .unwrap();
    assert_eq!(text.as_bytes(), exported);

    for kept in [
        "execution_run",
        "execution_evidence",
        "execution_response",
        "failed",
        "req-speech-failed",
        "speech_decode",
        "502",
        "audio.data",
        "PCM audio",
        "speech-model",
        "request-123",
        "input_tokens",
        "12",
        "Unclassified",
        "quota_exceeded",
        "0 credits remaining",
        "ELEVENLABS_HTTP_401",
        "eleven_v4_turbo",
        "f97fd76fcb72dfbd4ee6b07cc50867bc",
        "5ba1a5767b7f4da4a835e603361a2161",
    ] {
        assert!(text.contains(kept), "missing {kept}: {text}");
    }
    for private in [
        "source-canary",
        "credential-canary",
        "prompt-canary",
        "private-adapter-code",
        "private-adapter-path",
        "provisional-content-canary",
    ] {
        assert!(!text.contains(private), "leaked {private}");
    }
    let lines: Vec<serde_json::Value> = text
        .lines()
        .map(|line| serde_json::from_str(line).unwrap())
        .collect();
    let response = lines
        .iter()
        .find(|v| v["code"] == "execution_evidence")
        .unwrap();
    let transport = &lines
        .iter()
        .find(|v| {
            v["response"]["additional"]["transport"]["ClassifiedJson"]["http"]["status"] == 200
        })
        .unwrap()["response"]["additional"]["transport"]["ClassifiedJson"];
    assert_eq!(transport["http"]["status"], 200);
    assert_eq!(transport["diagnostics"]["error"]["status"], 502);
    assert_eq!(
        transport["diagnostics"]["error"]["diagnostics"]["status"],
        401
    );
    assert_eq!(response["state"], "failed");
    assert_eq!(response["complete"], true);
    assert!(response["execution"].is_string());
}
