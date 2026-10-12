use super::*;
use serde_json::json;
use std::sync::Mutex;

fn values(id: &str) -> Values {
    capture(
        SourceText {
            id: id.into(),
            text: "  cafe\u{301} العربية 日本語\n".into(),
        },
        Languages {
            source: "source-language".into(),
            destination: "destination-language".into(),
            destination_writing: vec!["Captured writing guidance.".into()],
        },
        &ResolvedTarget {
            audio_resolution: None,
            route: crate::model::ConnectionRoute::Custom,
            revision: 3,
            url: "http://127.0.0.1:12345/v1".into(),
            model: "captured-model".into(),
            credential: Some("credential-reference".into()),
        },
        "install",
    )
    .unwrap()
}

fn completion(source: &str, result: serde_json::Value) -> provider::Completion {
    provider::Completion {
        text: json!({"source":source,"translation":result}).to_string(),
        finish_reason: "stop".into(),
        actual_model: "served-model".into(),
        provider_id: "provider-request".into(),
        input_tokens: Some(17),
        output_tokens: Some(8),
        diagnostics: None,
    }
}

fn begin(graph: &Arc<Executable>, input: Values) -> Engine {
    let mut engine = Engine::new([graph.clone()]).unwrap();
    engine
        .apply(Event::Begin {
            run: "run".into(),
            artifact: graph.identity().into(),
            inputs: input,
            scope: "source-authority".into(),
            policy: BTreeMap::new(),
        })
        .unwrap();
    engine
}

fn advance(engine: &mut Engine) -> Vec<Work> {
    engine
        .apply(Event::Advance(Capacity {
            local: 2,
            provider: 2,
        }))
        .unwrap()
}

async fn execute(engine: &mut Engine, work: &Work) -> InvocationReport {
    engine
        .claim(work.execution)
        .unwrap()
        .execute_with_provisional(
            EvidenceLimits {
                observations: 16,
                bytes: 16384,
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

#[tokio::test]
async fn explicit_request_uses_captured_inputs_and_publishes_only_bound_validated_output() {
    let calls = Arc::new(Mutex::new(0));
    let count = calls.clone();
    let graph = Arc::new(
        compile(Arc::new(move |invocation, request| {
            *count.lock().unwrap() += 1;
            Box::pin(async move {
                let wire = request.text_request("wire-attempt".into(), "wire-operation".into());
                assert_eq!(wire.attempt, "wire-attempt");
                assert_eq!(wire.operation, "wire-operation");
                assert_eq!(wire.target.revision, 3);
                assert_eq!(wire.credential, "credential-reference");
                assert_eq!(wire.messages[1].content, request.source.text);
                assert!(
                    wire.messages[0]
                        .content
                        .ends_with("Captured writing guidance.")
                );
                assert_eq!(request.schema(), translation::schema());
                let completion = completion(&request.source.text, json!("Translated passage"));
                invocation.observe(graph_evidence::completion(
                    &completion,
                    &wire.model,
                    &[&request.source.text],
                ))?;
                Ok(completion)
            })
        }))
        .unwrap(),
    );
    let input = values("source-identity");
    let original = input["source"].clone();
    let mut engine = begin(&graph, input);
    assert!(advance(&mut engine).is_empty());
    assert_eq!(*calls.lock().unwrap(), 0);
    assert_eq!(
        engine
            .inspect("run")
            .unwrap()
            .artifact
            .definition
            .nodes
            .len(),
        1
    );
    engine
        .apply(Event::Demand {
            run: "run".into(),
            node: "translate".into(),
        })
        .unwrap();
    let work = advance(&mut engine);
    assert_eq!(work.len(), 1);
    let report = execute(&mut engine, &work[0]).await;
    assert!(report.outcome.is_ok());
    engine.apply(Event::SettleObserved(report)).unwrap();
    assert!(engine.outputs("run").unwrap().is_none());
    adopt(&mut engine, "translate");
    assert_eq!(
        engine.outputs("run").unwrap().unwrap()["translation"],
        json!({"source":original,"text":"Translated passage"})
    );
    assert!(advance(&mut engine).is_empty());
    assert_eq!(*calls.lock().unwrap(), 1);
}

#[tokio::test]
async fn unclear_and_foreign_responses_fail_with_evidence_without_implicit_retry() {
    for foreign in [false, true] {
        let graph = Arc::new(
            compile(Arc::new(move |invocation, request| {
                Box::pin(async move {
                    let output = if foreign {
                        completion("some other passage", json!("translation"))
                    } else {
                        completion(&request.source.text, serde_json::Value::Null)
                    };
                    invocation.observe(graph_evidence::completion(
                        &output,
                        &request.target.model,
                        &[&request.source.text],
                    ))?;
                    Ok(output)
                })
            }))
            .unwrap(),
        );
        let mut engine = begin(&graph, values("source"));
        engine
            .apply(Event::Demand {
                run: "run".into(),
                node: "translate".into(),
            })
            .unwrap();
        let work = advance(&mut engine);
        let report = execute(&mut engine, &work[0]).await;
        assert_eq!(
            report.outcome.as_ref().unwrap_err().code,
            "translation_response_invalid"
        );
        assert_eq!(
            report.observations[0].request_id.as_deref(),
            Some("provider-request")
        );
        assert_eq!(
            report.observations[0].actual_model.as_deref(),
            Some("served-model")
        );
        let evidence = serde_json::to_string(&report.observations).unwrap();
        assert!(!evidence.contains("some other passage"));
        assert!(!evidence.contains("日本語"));
        engine.apply(Event::SettleObserved(report)).unwrap();
        assert!(engine.outputs("run").unwrap().is_none());
        assert!(advance(&mut engine).is_empty());
        assert_eq!(
            engine.disposition("run", "translate").unwrap(),
            Disposition::Failed
        );
    }
}

#[tokio::test]
async fn learner_and_reply_nodes_share_one_operation_with_static_data_edges_and_separate_demand() {
    let calls = Arc::new(Mutex::new(Vec::new()));
    let seen = calls.clone();
    let provider: Provider = Arc::new(move |_, request| {
        seen.lock().unwrap().push(request.source.id.clone());
        Box::pin(async move { Ok(completion(&request.source.text, json!("translated"))) })
    });
    let mut registry = Registry::default();
    graph_text::register_types(&mut registry).unwrap();
    source_graph::register(&mut registry).unwrap();
    register(&mut registry, provider).unwrap();
    let source_ports = BTreeMap::from([("source".into(), required(source_contract()))]);
    registry
        .register(
            Operation {
                contract: contract("fixture.source"),
                implementation: "fixture/source/1".into(),
                inputs: source_ports.clone(),
                outputs: source_ports,
                resource: Resource::Local,
                reuse: Reuse::Fresh,
            },
            Arc::new(|_, values| Box::pin(async move { Ok(values) })),
        )
        .unwrap();
    let mut boundary = inputs();
    boundary.insert("reply_source".into(), required(source_contract()));
    let bindings: BTreeMap<_, _> = inputs()
        .keys()
        .map(|key| (key.clone(), Source::Input(key.clone())))
        .collect();
    let mut reply_bindings = bindings.clone();
    reply_bindings.insert(
        "source".into(),
        Source::Output {
            node: "reply".into(),
            port: "source".into(),
        },
    );
    let definition = Definition {
        contract: contract("fixture.conversation-translations"),
        inputs: boundary,
        outputs: BTreeMap::new(),
        results: BTreeMap::new(),
        compositions: BTreeMap::new(),
        nodes: BTreeMap::from([
            (
                "learner_translation".into(),
                Node {
                    operation: operation_contract(),
                    inputs: bindings,
                    after: vec![],
                    guard: None,
                    activation: Activation::Automatic,
                },
            ),
            (
                "reply".into(),
                Node {
                    operation: contract("fixture.source"),
                    inputs: BTreeMap::from([(
                        "source".into(),
                        Source::Input("reply_source".into()),
                    )]),
                    after: vec![],
                    guard: None,
                    activation: Activation::Automatic,
                },
            ),
            (
                "reply_translation".into(),
                Node {
                    operation: operation_contract(),
                    inputs: reply_bindings,
                    after: vec![],
                    guard: None,
                    activation: Activation::OnDemand,
                },
            ),
        ]),
    };
    let graph = Arc::new(registry.compile(definition).unwrap());
    let topology = graph.artifact().definition.clone();
    let mut input = values("learner");
    input.insert("reply_source".into(), values("partner")["source"].clone());
    let mut engine = begin(&graph, input);
    for work in advance(&mut engine) {
        let report = execute(&mut engine, &work).await;
        engine.apply(Event::SettleObserved(report)).unwrap();
    }
    adopt(&mut engine, "reply");
    adopt(&mut engine, "learner_translation");
    assert!(advance(&mut engine).is_empty());
    assert_eq!(*calls.lock().unwrap(), ["learner"]);
    assert_eq!(engine.inspect("run").unwrap().artifact.definition, topology);
    engine
        .apply(Event::Demand {
            run: "run".into(),
            node: "reply_translation".into(),
        })
        .unwrap();
    let work = advance(&mut engine);
    assert_eq!(work.len(), 1);
    let report = execute(&mut engine, &work[0]).await;
    engine.apply(Event::SettleObserved(report)).unwrap();
    adopt(&mut engine, "reply_translation");
    assert_eq!(*calls.lock().unwrap(), ["learner", "partner"]);
    assert_eq!(engine.inspect("run").unwrap().artifact.definition, topology);
}
