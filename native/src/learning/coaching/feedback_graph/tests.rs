use super::*;
use serde_json::json;

pub(crate) fn context(messages: Vec<provider::PromptMessage>) -> Context {
    Context {
        messages,
        input: InputEvidence::default(),
        target_language: "Spanish".into(),
        translation_language: "English".into(),
        candidate_constructs: vec![Candidate {
            id: "question".into(),
            criterion: "Ask a question.".into(),
        }],
        practice_settings: Practice {
            difficulty: "beginner".into(),
            coach_proactivity: "on_request".into(),
        },
        practice_focus: None,
        feedback_context: Some("I meant yesterday.".into()),
        feedback_policy: crate::configuration::Registry::bundled()
            .unwrap()
            .feedback_policy()
            .clone(),
        guidance: BTreeMap::from([(
            "assessment".into(),
            vec!["Captured assessment guidance".into()],
        )]),
    }
}

#[tokio::test]
async fn feedback_uses_shared_validation_and_retains_omission_diagnostics() {
    for correction in [false, true] {
        let mut captured = context(vec![provider::PromptMessage {
            role: "user".into(),
            content: "source".into(),
        }]);
        captured
            .feedback_policy
            .intensity
            .get_mut("light")
            .unwrap()
            .start_at = "hint".into();
        let source = SourceText {
            id: "learner".into(),
            text: "source".into(),
        };
        let target = ResolvedTarget {
            audio_resolution: None,
            route: crate::model::ConnectionRoute::Custom,
            revision: 3,
            url: "http://127.0.0.1:12345/v1".into(),
            model: "model".into(),
            credential: None,
        };
        let values = capture(source, captured, &target, "install").unwrap();
        let mut registry = Registry::default();
        graph_text::register_types(&mut registry).unwrap();
        source_graph::register(&mut registry).unwrap();
        register(&mut registry, Arc::new(move |_, request| Box::pin(async move {
            let wire = request.text_request("attempt".into(), "operation".into()).unwrap();
            let data: serde_json::Value = serde_json::from_str(&wire.messages[1].content).unwrap();
            assert_eq!(data["learnerSource"], "source");
            assert_eq!(data["learnerClarification"], "I meant yesterday.");
            assert_eq!(data["helpMode"], "hint");
            assert!(wire.messages[0].content.contains("Captured assessment guidance"));
            assert!(request.schema().unwrap()["properties"]["items"]["items"]["properties"]["error"]["properties"].get("hint").is_some());
            let error = if correction { json!({"op":"replace","category":"grammar","source":"unknown","blocks_meaning":true,"target_hypothesis":"replacement","hint":"Think about time."}) } else { json!(null) };
            Ok(provider::Completion {
                text:json!({"meaning_recovered":"full","items":[
                    {"construct":"question","quote":"source","outcome":if correction {"partial"} else {"demonstrated"},"error":error,"rationale":"Explanation"},
                    {"construct":"question","quote":" ","outcome":"demonstrated","error":null,"rationale":""}
                ]}).to_string(), finish_reason:"stop".into(),actual_model:"model".into(),provider_id:"request".into(),input_tokens:None,output_tokens:None,diagnostics:None,
            })
        }))).unwrap();
        let graph = Arc::new(
            registry
                .compile(Definition {
                    contract: Contract::new("test.feedback", 1),
                    inputs: inputs(),
                    outputs: BTreeMap::from([("feedback".into(), required(result_contract()))]),
                    nodes: BTreeMap::from([(
                        "feedback".into(),
                        Node {
                            operation: operation_contract(),
                            inputs: inputs()
                                .keys()
                                .map(|name| (name.clone(), Source::Input(name.clone())))
                                .collect(),
                            after: vec![],
                            guard: None,
                            activation: Activation::Automatic,
                        },
                    )]),
                    results: BTreeMap::from([(
                        "feedback".into(),
                        Source::Output {
                            node: "feedback".into(),
                            port: "feedback".into(),
                        },
                    )]),
                    compositions: BTreeMap::new(),
                })
                .unwrap(),
        );
        let mut engine = Engine::new([graph.clone()]).unwrap();
        engine
            .apply(Event::Begin {
                run: "run".into(),
                artifact: graph.identity().into(),
                inputs: values.clone(),
                scope: "scope".into(),
                policy: BTreeMap::new(),
            })
            .unwrap();
        let work = engine
            .apply(Event::Advance(Capacity {
                local: 1,
                provider: 1,
            }))
            .unwrap();
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
        let attempt = engine.inspect("run").unwrap().attempts["feedback"][0].id;
        engine
            .apply(Event::Adopt {
                run: "run".into(),
                node: "feedback".into(),
                attempt,
            })
            .unwrap();
        let output = engine.outputs("run").unwrap().unwrap();
        assert_eq!(output["feedback"]["source"], values["source"]);
        let feedback = &output["feedback"]["value"];
        assert_eq!(feedback["validationOmissions"], 1);
        assert_eq!(
            feedback["observation"]["items"].as_array().unwrap().len(),
            1
        );
        if correction {
            assert_eq!(feedback["decision"]["shown"]["text"], "Think about time.");
            assert!(feedback["decision"]["shown"]["explanation"].is_null());
        } else {
            assert!(feedback["decision"]["shown"].is_null());
        }
    }
}
