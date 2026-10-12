use super::*;
use serde_json::json;

fn values() -> Values {
    capture(
        Task::Brief,
        SourceText {
            id: "partner".into(),
            text: "¿Qué quieres cocinar?".into(),
        },
        None,
        Context {
            messages: vec![provider::PromptMessage {
                role: "system".into(),
                content: "Captured instructions".into(),
            }],
            target_language: "spanish".into(),
            translation_language: "english".into(),
            language_context: Language {
                script: "latin".into(),
                guidance: BTreeMap::from([(
                    "explanation_writing".into(),
                    vec!["Captured explanation guidance".into()],
                )]),
            },
            practice_settings: Practice {
                difficulty: "beginner".into(),
            },
            input: None,
        },
        &ResolvedTarget {
            audio_resolution: None,
            route: crate::model::ConnectionRoute::Custom,
            revision: 3,
            url: "http://127.0.0.1:12345/v1".into(),
            model: "model".into(),
            credential: None,
        },
        "install",
    )
    .unwrap()
}
fn completion(value: serde_json::Value) -> provider::Completion {
    provider::Completion {
        text: value.to_string(),
        finish_reason: "stop".into(),
        actual_model: "model".into(),
        provider_id: "request".into(),
        input_tokens: None,
        output_tokens: None,
        diagnostics: None,
    }
}
fn definition(ports: Ports, operations: Vec<(&str, Contract, Contract)>) -> Definition {
    let mut definition = Definition {
        contract: Contract::new("test.support", 1),
        inputs: ports.clone(),
        outputs: BTreeMap::new(),
        nodes: BTreeMap::new(),
        results: BTreeMap::new(),
        compositions: BTreeMap::new(),
    };
    for (name, operation, result) in operations {
        definition.nodes.insert(
            name.into(),
            Node {
                operation,
                inputs: ports
                    .keys()
                    .map(|name| (name.clone(), Source::Input(name.clone())))
                    .collect(),
                after: vec![],
                guard: None,
                activation: Activation::OnDemand,
            },
        );
        definition.outputs.insert(name.into(), required(result));
        definition.results.insert(
            name.into(),
            Source::Output {
                node: name.into(),
                port: "support".into(),
            },
        );
    }
    definition
}
async fn execute(engine: &mut Engine, node: &str) {
    engine
        .apply(Event::Demand {
            run: "run".into(),
            node: node.into(),
        })
        .unwrap();
    let work = engine
        .apply(Event::Advance(Capacity {
            local: 2,
            provider: 2,
        }))
        .unwrap();
    assert_eq!(work.len(), 1);
    let report = engine
        .claim(work[0].execution)
        .unwrap()
        .execute(EvidenceLimits {
            observations: 16,
            bytes: 16384,
        })
        .await;
    assert!(report.outcome.is_ok(), "{:?}", report.outcome);
    engine.apply(Event::SettleObserved(report)).unwrap();
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
fn begin(graph: Arc<Executable>, inputs: Values) -> Engine {
    let mut engine = Engine::new([graph.clone()]).unwrap();
    engine
        .apply(Event::Begin {
            run: "run".into(),
            artifact: graph.identity().into(),
            inputs,
            scope: "scope".into(),
            policy: BTreeMap::new(),
        })
        .unwrap();
    engine
}

#[tokio::test]
async fn support_operations_use_actual_reply_and_only_run_when_demanded() {
    let mut registry = Registry::default();
    graph_text::register_types(&mut registry).unwrap();
    source_graph::register(&mut registry).unwrap();
    register(&mut registry, Arc::new(|_, request| Box::pin(async move {
        let wire = request.text_request("attempt".into(), "operation".into()).unwrap();
        let data: serde_json::Value = serde_json::from_str(&wire.messages[1].content).unwrap();
        assert_eq!(data["actualPartnerReply"], request.source.text);
        assert!(data["latestLearnerInput"].is_null());
        let value = match request.task {
            Task::Brief => json!({"explanation":"The partner asks what you want to cook."}),
            Task::Assistance => {
                assert_eq!(request.schema()["properties"]["replies"]["items"]["properties"]["romanization"]["enum"], json!([""]));
                json!({"replies":[{"text":"Quiero cocinar arroz.","translation":"I want to cook rice.","romanization":"","pronunciation":"kee-eh-ro ko-see-nar a-ros"},{"text":"Quiero cocinar sopa.","translation":"I want to cook soup.","romanization":"","pronunciation":"kee-eh-ro ko-see-nar so-pa"}],"frames":["Quiero ___.","Prefiero ___."],"starters":["Quiero", "Prefiero"]})
            }
        };
        Ok(completion(value))
    }))).unwrap();
    let graph = Arc::new(
        registry
            .compile(definition(
                inputs(),
                vec![
                    ("brief", Task::Brief.operation(), Task::Brief.result()),
                    (
                        "assistance",
                        Task::Assistance.operation(),
                        Task::Assistance.result(),
                    ),
                ],
            ))
            .unwrap(),
    );
    let mut engine = begin(graph.clone(), values());
    assert!(
        engine
            .apply(Event::Advance(Capacity {
                local: 2,
                provider: 2
            }))
            .unwrap()
            .is_empty()
    );
    execute(&mut engine, "brief").await;
    execute(&mut engine, "assistance").await;
    let output = engine.outputs("run").unwrap().unwrap();
    assert_eq!(output["brief"]["source"], output["assistance"]["source"]);
    assert_eq!(
        engine.inspect("run").unwrap().artifact.definition,
        graph.artifact().definition
    );
}
#[test]
fn support_source_and_learner_identity_cannot_be_aliased() {
    let mut input = values();
    input.insert("learner".into(), json!({"id":"partner","text":"source"}));
    assert_eq!(decode(Task::Brief, &input).err().unwrap().path, "source");
}

#[tokio::test]
async fn grammar_snapshots_available_evidence_without_waiting_for_assessment() {
    for localized in [
        None,
        Some(vec![]),
        Some(vec![json!({"quote":"ayer","start":3,"end":7})]),
    ] {
        let mut input = values();
        input.insert("learner".into(), json!({"id":"learner","text":"🙂 ayer"}));
        input.get_mut("context").unwrap()["messages"] = json!([
            {"role":"system","content":"Captured instructions"}, {"role":"user","content":"🙂 ayer"}
        ]);
        let spans = localized
            .clone()
            .map(|spans| json!({"past":spans}))
            .unwrap_or(json!({}));
        input.insert("learning".into(), json!({
            "skills":[{"id":"past","name":"Past events","overview":"Locate an event before now."}],
            "evidence":{"source":input["learner"],"presence":{"past":"direct"},"spans":spans}
        }));
        let expected = localized
            .clone()
            .map(|spans| json!(spans))
            .unwrap_or(json!(null));
        let mut registry = Registry::default();
        graph_text::register_types(&mut registry).unwrap();
        source_graph::register(&mut registry).unwrap();
        contracts::register(&mut registry).unwrap();
        explanation::register(&mut registry, Arc::new(move |_, request| {
            let expected = expected.clone();
            Box::pin(async move {
                let wire = request.text_request("attempt".into(), "operation".into()).unwrap();
                let data: serde_json::Value = serde_json::from_str(&wire.messages[1].content).unwrap();
                assert_eq!(data["learnerSkillEvidence"][0]["spans"], expected);
                assert_eq!(data["learnerSkillEvidence"][0]["source"], "latestLearnerInput");
                assert_eq!(data["actualPartnerReply"], "¿Qué quieres cocinar?");
                assert_eq!(data["skillDefinitions"][0]["name"], "Past events");
                Ok(completion(json!({"cards":[{"quote":"Qué","title":"Question word","body":"Qué asks what.","example":"¿Qué quieres?","contrast":""}]})))
            })
        })).unwrap();
        let graph = Arc::new(
            registry
                .compile(definition(
                    explanation::inputs(),
                    vec![(
                        "explain",
                        explanation::operation_contract(),
                        explanation::result_contract(),
                    )],
                ))
                .unwrap(),
        );
        let mut engine = begin(graph, input.clone());
        execute(&mut engine, "explain").await;
        assert_eq!(
            engine.outputs("run").unwrap().unwrap()["explain"]["source"],
            input["source"]
        );
        input.get_mut("learning").unwrap()["evidence"]["source"]["id"] = json!("partner");
        assert_eq!(
            explanation::decode(&input).err().unwrap().path,
            "learning.source"
        );
        input.get_mut("learning").unwrap()["evidence"] = json!(null);
        let request = explanation::decode(&input).unwrap();
        let wire = request
            .text_request("attempt".into(), "operation".into())
            .unwrap();
        let data: serde_json::Value = serde_json::from_str(&wire.messages[1].content).unwrap();
        assert_eq!(data["learnerSkillEvidence"], json!([]));
    }
}
