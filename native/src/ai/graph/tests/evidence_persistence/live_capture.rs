use super::*;
use std::sync::Mutex;

#[tokio::test]
async fn actual_pending_handler_can_checkpoint_evidence_before_its_final_result() {
    let (send_prefix, receive_prefix) = tokio::sync::oneshot::channel();
    let (release, wait) = tokio::sync::oneshot::channel();
    let channels = Mutex::new(Some((send_prefix, wait)));
    let mut registry = registry();
    registry
        .operations
        .get_mut(&contract("increment"))
        .unwrap()
        .1 = Arc::new(move |context, values| {
        let (send, wait) = channels.lock().unwrap().take().unwrap();
        Box::pin(async move {
            context.observe(ResponseEvidence {
                request_id: Some("in-flight".into()),
                ..ResponseEvidence::default()
            })?;
            context.provisional_text("海 early")?;
            send.send((context.snapshot()?, context.clone()))
                .ok()
                .unwrap();
            wait.await.unwrap();
            context.provisional_text("é corrected")?;
            context.observe(ResponseEvidence {
                finish_reason: Some("stop".into()),
                ..ResponseEvidence::default()
            })?;
            Ok(values)
        })
    });
    let graph = Arc::new(registry.compile(definition()).unwrap());
    let dir = tempfile::tempdir().unwrap();
    let mut store = SqlStore::open(&dir.path().join("owner.db"));
    let mut host = DurableEngine::create([graph.clone()], limits(), &mut store).unwrap();
    begin_host(&mut host, &mut store, &graph, "a");
    host.apply(capacity(1), &mut store).unwrap();
    let attempt = host.inspect("a").unwrap().attempts["first"][0].id;
    let invocation = host.claim("a", "first", attempt, &mut store).unwrap();
    let task = tokio::spawn(
        invocation.execute_with_provisional(evidence_limits(), ProvisionalLimits { bytes: 100 }),
    );
    let (prefix, callback) = receive_prefix.await.unwrap();
    assert_eq!(prefix.provisional.as_ref().unwrap().sequence, 1);
    assert_eq!(
        prefix.identity.engine.as_deref(),
        Some(host.stamp().engine.as_str())
    );
    assert!(!task.is_finished());
    let read_limits = LiveReadLimits {
        export: ExportLimits {
            bytes: 100_000,
            attempts: 100,
        },
        captures: 10,
        capture_bytes: 100_000,
    };
    let live = host
        .read_live_inspection("a", std::slice::from_ref(&prefix), read_limits, &mut store)
        .unwrap();
    assert_eq!(live.previews["first"].capture.text, "海 early");
    assert!(live.previews["first"].live && live.previews["first"].uncommitted);
    store.fail_before_commit = true;
    assert!(host.record_observations(&prefix, &mut store).is_err());
    assert!(
        host.read_execution_evidence(prefix.identity.execution, &mut store)
            .unwrap()
            .is_none()
    );
    store.fail_before_commit = false;
    host.record_observations(&prefix, &mut store).unwrap();
    let saved = host
        .read_execution_evidence(prefix.identity.execution, &mut store)
        .unwrap()
        .unwrap();
    assert!(!saved.complete);
    assert_eq!(saved.observations, prefix.observations);
    assert_eq!(saved.provisional, prefix.provisional);
    assert_eq!(
        host.inspect("a").unwrap().nodes["first"],
        Disposition::Running
    );
    release.send(()).unwrap();
    let report = task.await.unwrap();
    assert_eq!(
        callback.provisional_text("too late").unwrap_err().code,
        "invocation_closed"
    );
    assert_eq!(report.provisional.as_ref().unwrap().text, "é corrected");
    assert_eq!(report.provisional.as_ref().unwrap().sequence, 2);
    host.settle_report(&report, &mut store).unwrap();
    let saved = host
        .read_execution_evidence(prefix.identity.execution, &mut store)
        .unwrap()
        .unwrap();
    assert!(saved.complete);
    assert_eq!(saved.observations.len(), 2);
    assert_eq!(saved.provisional, report.provisional);
    assert_eq!(
        host.inspect("a").unwrap().nodes["first"],
        Disposition::Available
    );
    assert!(store.publications().is_empty());
    let settled = host
        .read_live_inspection("a", &[prefix], read_limits, &mut store)
        .unwrap();
    assert_eq!(settled.previews["first"].capture.text, "é corrected");
    assert!(!settled.previews["first"].live && !settled.previews["first"].uncommitted);
    assert!(settled.previews["first"].complete);
}
