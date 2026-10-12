use super::*;
use crate::ai::transport::provider::{Completion, PromptMessage};
use serde_json::json;
use std::sync::Mutex;

fn captured(kind: context::Kind) -> Captured {
    let mut messages = vec![PromptMessage {
        role: "system".into(),
        content: "Captured partner instructions".into(),
    }];
    if !matches!(kind, context::Kind::Opening) {
        messages.push(PromptMessage {
            role: "user".into(),
            content: "¿Cómo estás?".into(),
        });
    }
    let target = ResolvedTarget {
        audio_resolution: None,
        route: crate::model::ConnectionRoute::Custom,
        revision: 7,
        url: "http://127.0.0.1:12345/v1".into(),
        model: "reply-model".into(),
        credential: None,
    };
    Captured {
        context: context::Captured {
            kind,
            messages,
            source_ids: vec![],
        },
        learner_source: matches!(kind, context::Kind::Reply).then(|| SourceText {
            id: "learner-message".into(),
            text: "¿Cómo estás?".into(),
        }),
        reply_source_id: "reserved-reply-message".into(),
        gloss_settings: crate::language::linguistics::adapter::settings::Settings {
            target_language: "spanish".into(),
            explanation_language: "english".into(),
            explanation_writing: vec![],
            romanization: vec![],
            segmentation: vec![],
            romanization_enabled: false,
        },
        gloss_target: ResolvedTarget {
            model: "gloss-model".into(),
            ..target.clone()
        },
        languages: Languages {
            source: "spanish".into(),
            destination: "english".into(),
            destination_writing: vec![],
        },
        reading_target: ResolvedTarget {
            model: "reading-model".into(),
            ..target.clone()
        },
        reply_target: target,
        install_id: "install".into(),
    }
}

fn provider(calls: Arc<Mutex<Vec<String>>>) -> translation::Provider {
    Arc::new(move |_, request| {
        calls.lock().unwrap().push(request.source.id.clone());
        Box::pin(async move {
            assert_eq!(request.target.model, "reading-model");
            assert_eq!(request.temperature, 0.7);
            if request.source.id == "reserved-reply-message" {
                assert_eq!(request.source.text, "¡Muy bien! ¿Y tú?");
            }
            Ok(Completion {
                text: json!({"source":request.source.text,"translation":"Translated text"})
                    .to_string(),
                finish_reason: "stop".into(),
                actual_model: "reading-model".into(),
                provider_id: "request".into(),
                input_tokens: None,
                output_tokens: None,
                diagnostics: None,
            })
        })
    })
}

#[test]
fn capture_rejects_missing_foreign_or_aliased_learner_sources() {
    for defect in ["missing", "foreign", "aliased", "opening"] {
        let mut value = captured(context::Kind::Reply);
        match defect {
            "missing" => value.learner_source = None,
            "foreign" => value.learner_source.as_mut().unwrap().text = "Other passage".into(),
            "aliased" => value.learner_source.as_mut().unwrap().id = value.reply_source_id.clone(),
            "opening" => value.context.kind = context::Kind::Opening,
            _ => unreachable!(),
        }
        let error = capture(value).unwrap_err();
        assert_eq!(error.code, "reply_source_invalid");
        assert_eq!(error.path, "learner_source");
    }
}

async fn drive(engine: &mut Engine) {
    // A finite fixture driver uses actual native preparation/dispatch/settlement/
    // adoption. No node-name scheduling or duplicated dependency rules.
    for _ in 0..12 {
        let work = engine
            .apply(Event::Advance(Capacity {
                local: 4,
                provider: 4,
            }))
            .unwrap();
        if work.is_empty() {
            return;
        }
        for work in work {
            let report = engine
                .claim(work.execution)
                .unwrap()
                .execute_with_provisional(
                    EvidenceLimits {
                        observations: 16,
                        bytes: 16384,
                    },
                    ProvisionalLimits { bytes: 4096 },
                )
                .await;
            assert!(report.outcome.is_ok(), "{:?}", report.outcome);
            engine.apply(Event::SettleObserved(report)).unwrap();
        }
        let ready: Vec<_> = engine
            .inspect("run")
            .unwrap()
            .attempts
            .iter()
            .filter_map(|(node, attempts)| {
                attempts
                    .last()
                    .filter(|a| matches!(a.state, AttemptState::Available))
                    .map(|a| (node.clone(), a.id))
            })
            .collect();
        for (node, attempt) in ready {
            engine
                .apply(Event::Adopt {
                    run: "run".into(),
                    node,
                    attempt,
                })
                .unwrap();
        }
    }
    panic!("native component failed to reach a fixed point");
}

#[tokio::test]
async fn reply_and_openings_execute_the_same_operations_with_explicit_source_bindings() {
    for kind in [
        context::Kind::Reply,
        context::Kind::Opening,
        context::Kind::SeededOpening,
    ] {
        for automatic in [false, true] {
            let calls = Arc::new(Mutex::new(Vec::new()));
            let graph = Arc::new(
                compile(
                    kind,
                    Arc::new(|_, request| {
                        Box::pin(async move {
                            context::validate(&request.context).unwrap();
                            assert_eq!(request.target.model, "reply-model");
                            assert_eq!(request.temperature, 1.1);
                            Ok("¡Muy bien! ¿Y tú?".into())
                        })
                    }),
                    provider(calls.clone()),
                    gloss_provider(),
                )
                .unwrap(),
            );
            let learner = matches!(kind, context::Kind::Reply);
            let mut policy = BTreeMap::new();
            if automatic {
                policy.insert("reply_translation".into(), Activation::Automatic);
                policy.insert("reply_gloss".into(), Activation::Automatic);
                if learner {
                    policy.insert("learner_translation".into(), Activation::Automatic);
                    policy.insert("learner_gloss".into(), Activation::Automatic);
                }
            }
            let mut engine = Engine::new([graph.clone()]).unwrap();
            engine
                .apply(Event::Begin {
                    run: "run".into(),
                    artifact: graph.identity().into(),
                    inputs: capture(captured(kind)).unwrap(),
                    scope: "scope".into(),
                    policy,
                })
                .unwrap();
            drive(&mut engine).await;
            let view = engine.inspect("run").unwrap();
            assert_eq!(
                view.artifact.definition.nodes.len(),
                if learner { 7 } else { 5 }
            );
            assert_eq!(view.artifact.definition, graph.artifact().definition);
            if !automatic {
                assert!(calls.lock().unwrap().is_empty());
                engine
                    .apply(Event::Demand {
                        run: "run".into(),
                        node: "reply_translation".into(),
                    })
                    .unwrap();
                if learner {
                    engine
                        .apply(Event::Demand {
                            run: "run".into(),
                            node: "learner_translation".into(),
                        })
                        .unwrap();
                }
                drive(&mut engine).await;
            }
            if !automatic {
                engine
                    .apply(Event::Demand {
                        run: "run".into(),
                        node: "reply_gloss".into(),
                    })
                    .unwrap();
                if learner {
                    engine
                        .apply(Event::Demand {
                            run: "run".into(),
                            node: "learner_gloss".into(),
                        })
                        .unwrap();
                }
                drive(&mut engine).await;
            }
            let output = engine.outputs("run").unwrap().unwrap();
            assert_eq!(output["reply_gloss"]["source"], output["reply_source"]);
            if learner {
                assert_eq!(output["learner_gloss"]["source"]["id"], "learner-message");
            }
            assert_eq!(output["text"], "¡Muy bien! ¿Y tú?");
            assert_eq!(
                output["reply_source"],
                json!({"id":"reserved-reply-message","text":"¡Muy bien! ¿Y tú?"})
            );
            assert_eq!(
                output["reply_translation"]["source"],
                output["reply_source"]
            );
            if learner {
                assert_eq!(
                    output["learner_translation"]["source"]["id"],
                    "learner-message"
                );
            }
            assert_eq!(calls.lock().unwrap().len(), if learner { 2 } else { 1 });
            assert_eq!(
                engine.inspect("run").unwrap().artifact.definition,
                graph.artifact().definition
            );
        }
    }
}

#[tokio::test]
async fn enclosing_graph_composes_and_executes_without_redeclaring_operations() {
    let calls = Arc::new(Mutex::new(Vec::new()));
    let mut registry = Registry::default();
    register(
        &mut registry,
        Arc::new(|_, _| Box::pin(async { Ok("¡Muy bien! ¿Y tú?".into()) })),
        provider(calls.clone()),
        gloss_provider(),
    )
    .unwrap();
    let child = registry.compile(definition(context::Kind::Reply)).unwrap();
    let source = definition(context::Kind::Reply);
    let bindings = source
        .inputs
        .keys()
        .map(|name| (name.clone(), Source::Input(name.clone())))
        .collect();
    let mut parent = Definition {
        contract: Contract::new("test.enclosing-conversation", 1),
        inputs: source.inputs,
        outputs: source.outputs,
        nodes: BTreeMap::new(),
        results: BTreeMap::new(),
        compositions: BTreeMap::new(),
    };
    parent.results = parent.compose("exchange", &child, bindings).unwrap();
    let graph = Arc::new(registry.compile(parent).unwrap());
    let nodes = &graph.artifact().definition.nodes;
    assert_eq!(nodes.len(), 7);
    assert_eq!(
        nodes["exchange/learner_translation"].after,
        vec!["exchange/context"]
    );
    assert_eq!(
        nodes["exchange/reply_translation"].inputs["source"],
        output("exchange/reply_source", "source")
    );
    assert_eq!(
        nodes["exchange/learner_translation"].operation,
        nodes["exchange/reply_translation"].operation
    );
    let mut engine = Engine::new([graph.clone()]).unwrap();
    engine
        .apply(Event::Begin {
            run: "run".into(),
            artifact: graph.identity().into(),
            inputs: capture(captured(context::Kind::Reply)).unwrap(),
            scope: "scope".into(),
            policy: BTreeMap::from([
                ("exchange/learner_translation".into(), Activation::Automatic),
                ("exchange/reply_translation".into(), Activation::Automatic),
                ("exchange/learner_gloss".into(), Activation::Automatic),
                ("exchange/reply_gloss".into(), Activation::Automatic),
            ]),
        })
        .unwrap();
    drive(&mut engine).await;
    let outputs = engine.outputs("run").unwrap().unwrap();
    assert_eq!(outputs["text"], "¡Muy bien! ¿Y tú?");
    assert_eq!(
        outputs["reply_translation"]["source"],
        outputs["reply_source"]
    );
    assert_eq!(calls.lock().unwrap().len(), 2);
    assert_eq!(
        engine.inspect("run").unwrap().artifact.definition,
        graph.artifact().definition
    );
}

#[tokio::test]
async fn context_failure_prevents_reply_and_learner_translation_even_when_automatic() {
    let graph = Arc::new(
        compile(
            context::Kind::Reply,
            Arc::new(|_, _| panic!("invalid context reached reply provider")),
            Arc::new(|_, _| panic!("invalid context reached translation provider")),
            Arc::new(|_, _| panic!("invalid context reached gloss provider")),
        )
        .unwrap(),
    );
    let mut captured = captured(context::Kind::Reply);
    captured.context.messages[0].role = "assistant".into();
    let mut engine = Engine::new([graph.clone()]).unwrap();
    engine
        .apply(Event::Begin {
            run: "run".into(),
            artifact: graph.identity().into(),
            inputs: capture(captured).unwrap(),
            scope: "scope".into(),
            policy: BTreeMap::from([
                ("learner_translation".into(), Activation::Automatic),
                ("reply_translation".into(), Activation::Automatic),
                ("learner_gloss".into(), Activation::Automatic),
                ("reply_gloss".into(), Activation::Automatic),
            ]),
        })
        .unwrap();
    let work = engine
        .apply(Event::Advance(Capacity {
            local: 1,
            provider: 4,
        }))
        .unwrap();
    assert_eq!(work.len(), 1);
    assert_eq!(work[0].resource, Resource::Local);
    let report = engine
        .claim(work[0].execution)
        .unwrap()
        .execute_with_provisional(
            EvidenceLimits {
                observations: 16,
                bytes: 16384,
            },
            ProvisionalLimits { bytes: 4096 },
        )
        .await;
    assert!(report.outcome.is_err());
    engine.apply(Event::SettleObserved(report)).unwrap();
    assert!(
        engine
            .apply(Event::Advance(Capacity {
                local: 4,
                provider: 4
            }))
            .unwrap()
            .is_empty()
    );
    assert_eq!(
        engine
            .inspect("run")
            .unwrap()
            .artifact
            .definition
            .nodes
            .len(),
        7
    );
}

fn gloss_provider() -> crate::language::gloss_graph::Provider {
    Arc::new(|_, request| {
        Box::pin(async move {
            assert_eq!(request.target.model, "gloss-model");
            assert_eq!(request.temperature, 0.7);
            Ok(Completion {
                text: "{\"spans\":[]}".into(),
                finish_reason: "stop".into(),
                actual_model: "gloss-model".into(),
                provider_id: "gloss-request".into(),
                input_tokens: None,
                output_tokens: None,
                diagnostics: None,
            })
        })
    })
}
