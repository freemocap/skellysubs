use super::*;
use crate::ai::transport::provider::PromptMessage;
use serde_json::json;
use std::sync::{Arc, Mutex};

fn target() -> ResolvedTarget {
    ResolvedTarget {
        audio_resolution: None,
        route: crate::model::ConnectionRoute::Custom,
        revision: 4,
        url: "http://127.0.0.1:8765/v1".into(),
        model: "test-model".into(),
        credential: Some("credential-reference".into()),
    }
}

fn inputs() -> Values {
    capture(
        vec![
            PromptMessage {
                role: "system".into(),
                content: "Instructions".into(),
            },
            PromptMessage {
                role: "user".into(),
                content: "  cafe\u{301} العربية 日本語\n".into(),
            },
        ],
        vec!["source-identity".into()],
        &target(),
        "test-install",
    )
    .unwrap()
}

fn begin(graph: &Arc<Executable>, values: Values) -> Engine {
    let mut engine = Engine::new([graph.clone()]).unwrap();
    engine
        .apply(Event::Begin {
            run: "run".into(),
            artifact: graph.identity().into(),
            inputs: values,
            scope: "test-authority".into(),
            policy: BTreeMap::new(),
        })
        .unwrap();
    engine
}

fn capacity(provider: usize) -> Event {
    Event::Advance(Capacity { local: 1, provider })
}

async fn execute(engine: &mut Engine, work: &Work) -> InvocationReport {
    engine
        .claim(work.execution)
        .unwrap()
        .execute_with_provisional(
            EvidenceLimits {
                observations: 10,
                bytes: 8192,
            },
            ProvisionalLimits { bytes: 4096 },
        )
        .await
}

fn adopt(engine: &mut Engine, node: &str) {
    let attempt = engine.inspect("run").unwrap().attempts[node]
        .last()
        .unwrap()
        .id;
    engine
        .apply(Event::Adopt {
            run: "run".into(),
            node: node.into(),
            attempt,
        })
        .unwrap();
}

async fn prepare_reply(engine: &mut Engine) -> Work {
    let local = engine.apply(capacity(0)).unwrap();
    assert_eq!(local.len(), 1);
    assert_eq!(local[0].resource, Resource::Local);
    let report = execute(engine, &local[0]).await;
    engine.apply(Event::SettleObserved(report)).unwrap();
    assert!(
        engine.apply(capacity(1)).unwrap().is_empty(),
        "context must be adopted first"
    );
    adopt(engine, CONTEXT);
    assert!(engine.apply(capacity(0)).unwrap().is_empty());
    let work = engine.apply(capacity(1)).unwrap();
    assert_eq!(work.len(), 1);
    assert_eq!(work[0].resource, Resource::Provider);
    work.into_iter().next().unwrap()
}

#[tokio::test]
async fn executable_owns_dependency_identity_and_exact_transport_inputs() {
    let calls = Arc::new(Mutex::new(Vec::new()));
    let recorded = calls.clone();
    let graph = Arc::new(
        compile(Arc::new(move |invocation, request| {
            let recorded = recorded.clone();
            Box::pin(async move {
                assert_eq!(request.context.source_ids, ["source-identity"]);
                let identity = invocation.identity().clone();
                let wire = request.text_request(
                    "durable-wire-attempt".into(),
                    "durable-wire-operation".into(),
                );
                assert_eq!(wire.attempt, "durable-wire-attempt");
                assert_eq!(wire.operation, "durable-wire-operation");
                assert_eq!(wire.messages[1].content, "  cafe\u{301} العربية 日本語\n");
                assert_eq!(wire.credential, "credential-reference");
                assert_eq!(wire.target.revision, 4);
                assert_eq!(wire.temperature, 0.7);
                assert_eq!(wire.install_id, "test-install");
                assert_eq!(wire.model, "test-model");
                recorded.lock().unwrap().push(identity);
                invocation.observe(ResponseEvidence {
                    actual_model: Some("served-model".into()),
                    ..Default::default()
                })?;
                Ok("Réponse 日本語".into())
            })
        }))
        .unwrap(),
    );
    let definition = graph
        .inspection_definition(ExportLimits {
            bytes: 32_768,
            attempts: 10,
        })
        .unwrap();
    assert_eq!(definition.artifact.definition.nodes.len(), 2);
    assert_eq!(
        definition.artifact.definition.nodes[REPLY].inputs["context"],
        Source::Output {
            node: CONTEXT.into(),
            port: "context".into()
        }
    );
    assert!(calls.lock().unwrap().is_empty());
    let mut engine = begin(&graph, inputs());
    let work = prepare_reply(&mut engine).await;
    assert!(calls.lock().unwrap().is_empty());
    let report = execute(&mut engine, &work).await;
    assert_eq!(report.identity.execution, work.execution);
    assert_eq!(report.identity.artifact, graph.identity());
    assert_eq!(
        report.observations[0].actual_model.as_deref(),
        Some("served-model")
    );
    engine.apply(Event::SettleObserved(report)).unwrap();
    assert!(engine.outputs("run").unwrap().is_none());
    adopt(&mut engine, REPLY);
    assert_eq!(
        engine.outputs("run").unwrap().unwrap()[TEXT],
        "Réponse 日本語"
    );
    assert_eq!(calls.lock().unwrap().len(), 1);
    assert_eq!(engine.inspect("run").unwrap().artifact, graph.artifact());
}

#[tokio::test]
async fn failure_retains_producer_evidence_and_never_resends_implicitly() {
    let calls = Arc::new(Mutex::new(0));
    let recorded = calls.clone();
    let graph = Arc::new(
        compile(Arc::new(move |invocation, _| {
            *recorded.lock().unwrap() += 1;
            Box::pin(async move {
                invocation.observe(ResponseEvidence {
                    request_id: Some("provider-request".into()),
                    status: Some(503),
                    ..Default::default()
                })?;
                invocation.provisional_text("partial source")?;
                Err(Fault {
                    code: "fixture_transport_failure".into(),
                    path: "transport".into(),
                })
            })
        }))
        .unwrap(),
    );
    let mut engine = begin(&graph, inputs());
    let work = prepare_reply(&mut engine).await;
    let report = execute(&mut engine, &work).await;
    assert!(report.outcome.is_err());
    assert_eq!(
        report.observations[0].request_id.as_deref(),
        Some("provider-request")
    );
    assert!(report.provisional.is_some());
    engine.apply(Event::SettleObserved(report)).unwrap();
    assert_eq!(
        engine.disposition("run", REPLY).unwrap(),
        Disposition::Failed
    );
    assert_eq!(engine.inspect("run").unwrap().artifact, graph.artifact());
    assert!(engine.outputs("run").unwrap().is_none());
    assert!(engine.apply(capacity(1)).unwrap().is_empty());
    assert_eq!(*calls.lock().unwrap(), 1);
    engine
        .apply(Event::Retry {
            run: "run".into(),
            node: REPLY.into(),
        })
        .unwrap();
    let retry = engine.apply(capacity(1)).unwrap();
    assert_eq!(retry.len(), 1);
    assert_ne!(retry[0].execution, work.execution);
}

#[tokio::test]
async fn invalid_captured_target_never_reaches_transport() {
    let graph =
        Arc::new(compile(Arc::new(|_, _| panic!("invalid inputs reached provider"))).unwrap());
    for (field, replacement) in [
        ("temperature", json!("NaN")),
        ("temperature", json!("0.70")),
        ("temperature", json!("3")),
        ("install_id", json!("")),
        (
            "target",
            json!({"route":"custom","revision":4,"model":"x","url":"https://user:secret@example.com"}),
        ),
    ] {
        let mut values = inputs();
        values.insert(field.into(), replacement);
        let mut engine = begin(&graph, values);
        let work = prepare_reply(&mut engine).await;
        let report = execute(&mut engine, &work).await;
        assert_eq!(
            report.outcome.unwrap_err().code,
            "conversation_prose_input_invalid"
        );
        assert!(report.observations.is_empty());
    }
}

#[tokio::test]
async fn keyless_target_is_explicit_and_malformed_context_blocks_reply() {
    let mut target = target();
    target.credential = None;
    let mut values = capture(
        vec![PromptMessage {
            role: "user".into(),
            content: "missing system".into(),
        }],
        vec![],
        &target,
        "install",
    )
    .unwrap();
    assert!(!values.contains_key("credential"));
    let graph =
        Arc::new(compile(Arc::new(|_, _| panic!("invalid context reached provider"))).unwrap());
    let mut engine = begin(&graph, values.clone());
    let work = engine.apply(capacity(1)).unwrap();
    let report = execute(&mut engine, &work[0]).await;
    assert_eq!(
        report.outcome.as_ref().unwrap_err().code,
        "conversation_context_invalid"
    );
    engine.apply(Event::SettleObserved(report)).unwrap();
    assert!(engine.apply(capacity(1)).unwrap().is_empty());
    assert_eq!(
        engine
            .inspect("run")
            .unwrap()
            .artifact
            .definition
            .nodes
            .len(),
        2
    );
    values.get_mut("context").unwrap()["unexpected"] = json!(true);
    let mut engine = Engine::new([graph.clone()]).unwrap();
    assert!(
        engine
            .apply(Event::Begin {
                run: "malformed".into(),
                artifact: graph.identity().into(),
                inputs: values,
                scope: "test".into(),
                policy: BTreeMap::new()
            })
            .is_err()
    );
}
