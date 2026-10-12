use super::*;
use crate::{
    ai::transport::provider::{Completion, PromptMessage},
    language::{linguistics::adapter::settings::Settings, source_graph::SourceText},
    learning::{coaching::InputEvidence, practice_assessment::SkillPrompt, turn_assessment},
};
use serde_json::json;
use std::sync::{Arc, Mutex};

fn completed(value: serde_json::Value) -> Completion {
    Completion {
        text: value.to_string(),
        finish_reason: "stop".into(),
        actual_model: "model".into(),
        provider_id: "request".into(),
        input_tokens: Some(10),
        output_tokens: Some(10),
        diagnostics: None,
    }
}

#[tokio::test]
async fn composed_partner_exchange_runs_reading_and_assessment_from_one_native_artifact() {
    let registry = crate::configuration::Registry::bundled().unwrap();
    let files = crate::configuration::content_files::read(
        &std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../content"),
    )
    .unwrap();
    let content = assessment_graph::Content {
        skills: registry
            .shared_skills()
            .skills
            .iter()
            .map(|s| SkillPrompt {
                id: s.id.clone(),
                name: s.name.clone(),
                overview: s.overview.clone(),
                boundary: s.boundary.clone(),
                language_guidance: "Captured fixture guidance".into(),
            })
            .collect(),
        instructions: crate::configuration::authoring::prompts::instructions(&files).unwrap(),
        questions: crate::configuration::authoring::prompts::message_questions(&files).unwrap(),
    };
    for (kind, positive, fail_assessment) in [
        (context::Kind::Reply, true, false),
        (context::Kind::Reply, false, false),
        (context::Kind::Reply, false, true),
        (context::Kind::Opening, false, false),
    ] {
        let calls = Arc::new(Mutex::new(Vec::new()));
        let assessment_calls = calls.clone();
        let attribution_calls = Arc::new(Mutex::new(0));
        let attribution_count = attribution_calls.clone();
        let graph = Arc::new(compile(kind, Providers {
        speech: Arc::new(|_, _| Box::pin(async { panic!("Speech is not requested by this fixture") })),
        audio_lookup: Arc::new(|_, _| Box::pin(async { panic!("Speech is not requested by this fixture") })),
            available_evidence: Arc::new(|_, _| Box::pin(async { Ok(None) })),
            explanations: Arc::new(|_, request| Box::pin(async move {
                assert!(request.learning.evidence.is_none());
                assert_eq!(request.exchange.source.id, "reply");
                request.text_request("attempt".into(),"operation".into()).unwrap();
                Ok(completed(json!({"cards":[{"quote":"Hola","title":"Greeting","body":"This is a greeting.","example":"Hola, Ana.","contrast":""}]})))
            })),
            feedback: Arc::new(|_, request| Box::pin(async move {
                assert_eq!(request.source.id, "learner");
                request.text_request("attempt".into(), "operation".into()).unwrap();
                Ok(completed(json!({"meaning_recovered":"full","items":[]})))
            })),
            support: Arc::new(|_, request| Box::pin(async move {
                let wire = request.text_request("attempt".into(), "operation".into()).unwrap();
                assert_eq!(serde_json::from_str::<serde_json::Value>(&wire.messages[1].content).unwrap()["actualPartnerReply"], "Hola.");
                assert_eq!(request.target.model, if request.task == support_graph::Task::Brief { "brief-model" } else { "assistance-model" });
                Ok(completed(match request.task {
                    support_graph::Task::Brief => json!({"explanation":"A greeting."}),
                    support_graph::Task::Assistance => json!({"replies":[{"text":"Hola.","translation":"Hello.","romanization":"","pronunciation":"oh-la"},{"text":"Buenos días.","translation":"Good morning.","romanization":"","pronunciation":"bweh-nos dee-as"}],"frames":["Hola ___.","Buenos ___."],"starters":["Hola", "Buenos días"]}),
                }))
            })),
            reply: Arc::new(|_, _| Box::pin(async { Ok("Hola.".into()) })),
            translation: Arc::new(|_, request| Box::pin(async move { Ok(completed(json!({"source":request.source.text,"translation":"Hello."}))) })),
            gloss: Arc::new(|_, _| Box::pin(async { Ok(completed(json!({"spans":[]}))) })),
            attribution: Arc::new(move |_, request| {
                *attribution_count.lock().unwrap() += 1;
                Box::pin(async move {
                    assert_eq!(request.target.model, "model");
                    assert_eq!(request.selection.source.id, "learner");
                    let wire = request.text_request("attempt".into(), "operation".into()).unwrap();
                    assert!(wire.decisions.is_none());
                    let skills: Vec<_> = request.selection.selected.iter().map(|id| json!({"skill_id":id,"spans":[{"quote":"días","occurrence":0}]})).collect();
                    Ok(completed(json!({"skills":skills})))
                })
            }),
            assessment: Arc::new(move |_, request| {
                assessment_calls.lock().unwrap().push(request.source.id.clone());
                Box::pin(async move {
                    if fail_assessment { return Err(Fault {code:"test_assessment_failed".into(),path:"assessment".into()}); }
                    let mut answers = json!({});
                    for skill in request.content.skills { answers[skill.id] = json!({"type":"choice","choice":if positive {"direct"} else {"absent"},"confidence":0.9,"probabilities":{"absent":0.05,"contextual":0.05,"direct":0.85,"unclear":0.05}}); }
                    for (name, labels) in [("grammar", turn_assessment::GRAMMAR_LABELS), ("understandability", turn_assessment::UNDERSTANDABILITY_LABELS)] {
                        let probabilities: BTreeMap<_, _> = labels.iter().map(|label| (*label, 0.25)).collect();
                        answers[name] = json!({"type":"choice","choice":labels[0],"confidence":0.75,"probabilities":probabilities});
                    }
                    Ok(completed(answers))
                })
            }),
        }).unwrap());
        let learner = matches!(kind, context::Kind::Reply);
        let mut messages = vec![PromptMessage {
            role: "system".into(),
            content: "Captured instructions".into(),
        }];
        if learner {
            messages.push(PromptMessage {
                role: "user".into(),
                content: "Buenos días.".into(),
            });
        }
        let target = ResolvedTarget {
            audio_resolution: None,
            route: crate::model::ConnectionRoute::Custom,
            revision: 3,
            url: "http://127.0.0.1:12345/v1".into(),
            model: "model".into(),
            credential: None,
        };
        let reply = reply_reading_graph::Captured {
            context: context::Captured {
                kind,
                messages: messages.clone(),
                source_ids: vec![],
            },
            learner_source: learner.then(|| SourceText {
                id: "learner".into(),
                text: "Buenos días.".into(),
            }),
            reply_source_id: "reply".into(),
            languages: translation_graph::Languages {
                source: "spanish".into(),
                destination: "english".into(),
                destination_writing: vec![],
            },
            gloss_settings: Settings {
                target_language: "spanish".into(),
                explanation_language: "english".into(),
                explanation_writing: vec![],
                romanization: vec![],
                segmentation: vec![],
                romanization_enabled: false,
            },
            gloss_target: target.clone(),
            reply_target: target.clone(),
            reading_target: target.clone(),
            install_id: "install".into(),
        };
        let support = support::Captured {
            skills: Some(vec![support_graph::explanation::Skill {
                id: "question".into(),
                name: "Questions".into(),
                overview: "Ask a question.".into(),
            }]),
            context: support_graph::Context {
                messages: messages.clone(),
                target_language: "spanish".into(),
                translation_language: "english".into(),
                language_context: support_graph::Language {
                    script: "latin".into(),
                    guidance: BTreeMap::new(),
                },
                practice_settings: support_graph::Practice {
                    difficulty: "beginner".into(),
                },
                input: learner.then(InputEvidence::default),
            },
            brief_target: ResolvedTarget {
                model: "brief-model".into(),
                ..target.clone()
            },
            assistance_target: ResolvedTarget {
                model: "assistance-model".into(),
                ..target.clone()
            },
        };
        let feedback = learner.then(|| feedback::Captured {
            context: feedback_graph::Context {
                messages: messages.clone(),
                input: InputEvidence::default(),
                target_language: "Spanish".into(),
                translation_language: "English".into(),
                candidate_constructs: vec![feedback_graph::Candidate {
                    id: "question".into(),
                    criterion: "Ask a question.".into(),
                }],
                practice_settings: feedback_graph::Practice {
                    difficulty: "beginner".into(),
                    coach_proactivity: "on_request".into(),
                },
                practice_focus: None,
                feedback_context: None,
                feedback_policy: registry.feedback_policy().clone(),
                guidance: BTreeMap::new(),
            },
            target: target.clone(),
        });
        let assessment = learner.then(|| AssessmentCapture {
            context: assessment_graph::Context {
                messages,
                input: InputEvidence::default(),
                language: "Spanish".into(),
                variety: "Captured variety".into(),
            },
            content: Some(content.clone()),
            target,
        });
        let mut engine = Engine::new([graph.clone()]).unwrap();
        let policy = graph
            .artifact()
            .definition
            .nodes
            .keys()
            .map(|name| {
                (
                    name.clone(),
                    if name.starts_with("speech/") {
                        Activation::Disabled
                    } else {
                        Activation::Automatic
                    },
                )
            })
            .collect();
        engine
            .apply(Event::Begin {
                run: "run".into(),
                artifact: graph.identity().into(),
                inputs: capture(reply, assessment, support, feedback).unwrap(),
                scope: "scope".into(),
                policy,
            })
            .unwrap();
        for _ in 0..12 {
            let work = engine
                .apply(Event::Advance(Capacity {
                    local: 8,
                    provider: 8,
                }))
                .unwrap();
            if work.is_empty() {
                break;
            }
            for item in work {
                let report = engine
                    .claim(item.execution)
                    .unwrap()
                    .execute(EvidenceLimits {
                        observations: 16,
                        bytes: 16384,
                    })
                    .await;
                assert!(
                    report.outcome.is_ok()
                        || (fail_assessment
                            && report
                                .outcome
                                .as_ref()
                                .err()
                                .is_some_and(|fault| fault.code == "test_assessment_failed")),
                    "{:?}",
                    report.outcome
                );
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
        if fail_assessment {
            assert_eq!(
                engine.disposition("run", "assessment").unwrap(),
                Disposition::Failed
            );
            assert_eq!(
                engine.disposition("run", "attribution").unwrap(),
                Disposition::Blocked
            );
            for node in [
                "reply",
                "brief",
                "feedback",
                "assistance",
                "explanation_evidence",
                "explanations",
            ] {
                assert_eq!(
                    engine.disposition("run", node).unwrap(),
                    Disposition::Adopted,
                    "{node}"
                );
            }
            continue;
        }
        let outputs = engine.outputs("run").unwrap().unwrap();
        assert_eq!(
            outputs["reply_gloss"]["source"],
            outputs["reply_translation"]["source"]
        );
        assert_eq!(outputs["brief"]["source"], outputs["reply_gloss"]["source"]);
        assert_eq!(
            outputs["explanations"]["source"],
            outputs["reply_gloss"]["source"]
        );
        assert_eq!(
            outputs["assistance"]["source"],
            outputs["reply_gloss"]["source"]
        );
        if learner {
            assert_eq!(
                outputs["feedback"]["source"],
                outputs["assessment"]["source"]
            );
            assert_eq!(
                outputs["assessment"]["source"],
                outputs["learner_translation"]["source"]
            );
            assert_eq!(
                outputs["assessment"]["assessment"]["grammar"]["confidence"],
                0.75
            );
        }
        assert_eq!(calls.lock().unwrap().len(), usize::from(learner));
        assert_eq!(
            *attribution_calls.lock().unwrap(),
            usize::from(learner && positive)
        );
        if learner && positive {
            assert_eq!(
                outputs["attribution"]["source"],
                outputs["assessment"]["source"]
            );
            for value in outputs["attribution"]["attribution"]["skills"]
                .as_object()
                .unwrap()
                .values()
            {
                assert_eq!(value["spans"][0]["quote"], "días");
                assert_eq!(value["spans"][0]["start"], 7);
            }
        } else {
            assert!(!outputs.contains_key("attribution"));
            if learner {
                assert_eq!(
                    engine.disposition("run", "attribution").unwrap(),
                    Disposition::Skipped
                );
            }
        }
        assert_eq!(
            engine.inspect("run").unwrap().artifact.definition,
            graph.artifact().definition
        );
    }
}
