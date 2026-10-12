//! Real application captures, native durable execution and product publication.
//! Admission/transport scheduling are tested separately when their host switches.
use super::*;
use crate::ai::{
    graph::*,
    graph_store::{self, Partition, TransactionStore},
};
use crate::language::source_graph::SourceText;
use crate::learning::{coaching::support_graph, turn_assessment};
use serde_json::{Value, json};
use std::{collections::BTreeMap, sync::Arc};

mod verification;

fn completed(value: Value) -> Completion {
    Completion {
        text: value.to_string(),
        finish_reason: "stop".into(),
        actual_model: "fixture".into(),
        provider_id: "fixture".into(),
        input_tokens: Some(2),
        output_tokens: Some(3),
        diagnostics: None,
    }
}

fn providers() -> partner_graph::Providers {
    partner_graph::Providers {
        speech: Arc::new(|_, _| {
            Box::pin(async { panic!("Speech is not requested by this fixture") })
        }),
        audio_lookup: Arc::new(|_, _| {
            Box::pin(async { panic!("Speech is not requested by this fixture") })
        }),
        reply: Arc::new(|_, _| Box::pin(async { Ok("Hola.".into()) })),
        translation: Arc::new(|_, r| {
            Box::pin(async move {
                Ok(completed(
                    json!({"source":r.source.text,"translation":"Hello."}),
                ))
            })
        }),
        gloss: Arc::new(|_, _| Box::pin(async { Ok(completed(json!({"spans":[]}))) })),
        feedback: Arc::new(|_, request| {
            Box::pin(async move {
                let construct = &request.context.candidate_constructs[0].id;
                Ok(completed(json!({"meaning_recovered":"full","items":[
                    {"construct":construct,"quote":request.source.text,"outcome":"demonstrated","error":null,"rationale":"Recognized source"},
                    {"construct":construct,"quote":" ","outcome":"demonstrated","error":null,"rationale":""}
                ]})))
            })
        }),
        support: Arc::new(|_, r| {
            Box::pin(async move {
                Ok(completed(match r.task {
                    support_graph::Task::Brief => json!({"explanation":"A greeting."}),
                    support_graph::Task::Assistance => {
                        json!({"replies":[{"text":"Hola.","translation":"Hello.","romanization":"","pronunciation":"oh-la"},{"text":"Buenos días.","translation":"Good morning.","romanization":"","pronunciation":"bweh-nos dee-as"}],"frames":["Hola ___.","Buenos ___."],"starters":["Hola", "Buenos días"]})
                    }
                }))
            })
        }),
        explanations: Arc::new(|_, _| {
            Box::pin(async {
                Ok(completed(
                    json!({"cards":[{"quote":"Hola","title":"Greeting","body":"This is a greeting.","example":"Hola, Ana.","contrast":""}]}),
                ))
            })
        }),
        available_evidence: Arc::new(|_, _| Box::pin(async { Ok(None) })),
        attribution: Arc::new(|_, r| {
            Box::pin(async move {
                Ok(completed(
                    json!({"skills":r.selection.selected.iter().map(|id|json!({"skill_id":id,"spans":[]})).collect::<Vec<_>>()}),
                ))
            })
        }),
        assessment: Arc::new(|_, r| {
            Box::pin(async move {
                let mut answers = json!({});
                for skill in r.content.skills {
                    answers[skill.id] = json!({"type":"choice","choice":"direct","confidence":0.9,"probabilities":{"absent":0.05,"contextual":0.05,"direct":0.85,"unclear":0.05}});
                }
                for (name, labels) in [
                    ("grammar", turn_assessment::GRAMMAR_LABELS),
                    (
                        "understandability",
                        turn_assessment::UNDERSTANDABILITY_LABELS,
                    ),
                ] {
                    let probabilities: BTreeMap<_, _> =
                        labels.iter().map(|label| (*label, 0.25)).collect();
                    answers[name] = json!({"type":"choice","choice":labels[0],"confidence":0.75,"probabilities":probabilities});
                }
                Ok(completed(answers))
            })
        }),
    }
}

fn adapter<'a>(
    db: &'a mut Connection,
    partition: &Partition,
    turn: &'a str,
    registry: &'a crate::configuration::Registry,
    session: &'a str,
    work: Option<&'a Work>,
    reject: bool,
) -> TransactionStore<
    'a,
    impl FnMut(&Connection, &CommitRequest<'_>) -> crate::ai::graph::Result<()> + 'a,
> {
    TransactionStore::new(
        db.transaction().unwrap(),
        partition.clone(),
        graph_runtime::limits().checkpoint.bytes,
        move |db: &Connection, request: &CommitRequest<'_>| {
            let result = (|| -> crate::model::Result<()> {
                match &request.intent {
                    CommitIntent::Begin { authority, .. } => {
                        db.execute("INSERT INTO turn_execution_owners VALUES(?1,'graph','persona_reply',?2,?3,?4)",params![turn,request.next.stamp().engine,authority.run,authority.artifact])?;
                        graph_publication::declare_reply(db, request, "reply", "text")?;
                    }
                    CommitIntent::Adopt { authority, .. } => {
                        let authorize = |db: &Connection, authority: &Authority<'_>| {
                            if reject {
                                Err(AppError::new(ErrorCode::Conflict, "fixture rejection"))
                            } else if authority.node != Some("reply") {
                                graph_authority::check_helper(
                                    db,
                                    &request.next.stamp().engine,
                                    authority,
                                    work.unwrap(),
                                    graph_authority::Phase::Adoption,
                                )
                            } else {
                                Ok(())
                            }
                        };
                        if authority.node == Some("reply") {
                            graph_publication::publish_reply(db, request, authorize)?;
                        } else if !graph_publication::publish_helper(
                            db,
                            request,
                            work.unwrap(),
                            authorize,
                        )? {
                            graph_publication::publish_assessment(
                                db,
                                request,
                                work.unwrap(),
                                registry,
                                session,
                                authorize,
                            )?;
                        }
                    }
                    _ => (),
                }
                Ok(())
            })();
            result.map_err(|error| {
                eprintln!("publication fixture: {error:?}");
                Fault {
                    code: "test_publication_rejected".into(),
                    path: "owner".into(),
                }
            })
        },
    )
}

#[tokio::test]
async fn native_partner_results_publish_without_legacy_attempts_and_preserve_credit_on_restart() {
    let (_dir, mut store, conversation) = setup();
    let snapshot = store.snapshot().unwrap();
    let turn = capture_native_send(
        &store.connection,
        &store.config,
        &snapshot,
        &conversation,
        "Hola, ¿cómo estás?",
        snapshot.conversations[0].revision,
    )
    .unwrap();
    store
        .connection
        .execute(
            "UPDATE turns SET context=json_set(context,'$.input',json(?2)) WHERE id=?1",
            params![
                turn,
                serde_json::to_string(&crate::learning::coaching::InputEvidence::default())
                    .unwrap()
            ],
        )
        .unwrap();
    let (raw, install): (String, String) = store
        .connection
        .query_row(
            "SELECT t.context,l.id FROM turns t CROSS JOIN learner l WHERE t.id=?1",
            [&turn],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .unwrap();
    let captured: Value = serde_json::from_str(&raw).unwrap();
    let source = store
        .connection
        .query_row(
            "SELECT id,text FROM messages WHERE turn_id=?1 AND role='user'",
            [&turn],
            |r| {
                Ok(SourceText {
                    id: r.get(0)?,
                    text: r.get(1)?,
                })
            },
        )
        .unwrap();
    let inputs = partner_graph::captured_turn::inputs(
        context::Kind::Reply,
        &captured,
        Some(source),
        "native-reply".into(),
        install,
    )
    .unwrap();
    let graph = Arc::new(partner_graph::compile(context::Kind::Reply, providers()).unwrap());
    let partition = Partition {
        owner: crate::ai::graph_store::Owner::Conversation(conversation.clone()),
        catalog: graph_store::catalog_id([graph.identity()]).unwrap(),
    };
    let policy = graph
        .artifact()
        .definition
        .nodes
        .keys()
        .filter(|node| !node.starts_with("speech/"))
        .map(|node| (node.clone(), Activation::Automatic))
        .collect();
    let mut engine = DurableEngine::create_with_run(
        [graph.clone()],
        graph_runtime::limits(),
        Event::Begin {
            run: turn.clone(),
            artifact: graph.identity().into(),
            inputs,
            scope: "fixture".into(),
            policy,
        },
        &mut adapter(
            &mut store.connection,
            &partition,
            &turn,
            &store.config,
            &store.session_id,
            None,
            false,
        ),
    )
    .unwrap();
    for _ in 0..40 {
        engine
            .apply(
                Event::Advance(Capacity {
                    local: 16,
                    provider: 16,
                }),
                &mut adapter(
                    &mut store.connection,
                    &partition,
                    &turn,
                    &store.config,
                    &store.session_id,
                    None,
                    false,
                ),
            )
            .unwrap();
        let prepared: Vec<_> = engine
            .inspect(&turn)
            .unwrap()
            .attempts
            .iter()
            .filter_map(|(node, attempts)| {
                attempts
                    .last()
                    .filter(|a| a.state == AttemptState::Prepared)
                    .map(|a| (node.clone(), a.id, a.execution))
            })
            .collect();
        if prepared.is_empty() {
            break;
        }
        for (node, attempt, execution) in prepared {
            let report = engine
                .claim(
                    &turn,
                    &node,
                    attempt,
                    &mut adapter(
                        &mut store.connection,
                        &partition,
                        &turn,
                        &store.config,
                        &store.session_id,
                        None,
                        false,
                    ),
                )
                .unwrap()
                .execute(EvidenceLimits {
                    observations: 32,
                    bytes: 64_000,
                })
                .await;
            assert!(report.outcome.is_ok(), "{node}: {:?}", report.outcome);
            engine
                .apply(
                    Event::SettleObserved(report),
                    &mut adapter(
                        &mut store.connection,
                        &partition,
                        &turn,
                        &store.config,
                        &store.session_id,
                        None,
                        false,
                    ),
                )
                .unwrap();
            let work = engine
                .read_execution_work(
                    execution,
                    &mut graph_store::ReadStore::new(&mut store.connection, partition.clone())
                        .unwrap(),
                )
                .unwrap();
            if matches!(
                node.as_str(),
                "reply"
                    | "learner_translation"
                    | "reply_translation"
                    | "learner_gloss"
                    | "reply_gloss"
                    | "assessment"
                    | "feedback"
                    | "attribution"
                    | "brief"
                    | "assistance"
                    | "explanations"
            ) {
                let product_before = verification::product_state(&store.connection, &turn);
                let before = engine.stamp().clone();
                let rows: i64 = store
                    .connection
                    .query_row(
                        "SELECT count(*) FROM conversation_graph_assessments",
                        [],
                        |r| r.get(0),
                    )
                    .unwrap();
                assert!(
                    engine
                        .adopt(
                            &turn,
                            &node,
                            attempt,
                            &mut adapter(
                                &mut store.connection,
                                &partition,
                                &turn,
                                &store.config,
                                &store.session_id,
                                Some(&work),
                                true
                            )
                        )
                        .is_err()
                );
                assert_eq!(engine.stamp(), &before);
                assert_eq!(
                    verification::product_state(&store.connection, &turn),
                    product_before
                );
                assert_eq!(
                    store
                        .connection
                        .query_row(
                            "SELECT count(*) FROM conversation_graph_assessments",
                            [],
                            |r| r.get::<_, i64>(0)
                        )
                        .unwrap(),
                    rows
                );
            }
            engine
                .adopt(
                    &turn,
                    &node,
                    attempt,
                    &mut adapter(
                        &mut store.connection,
                        &partition,
                        &turn,
                        &store.config,
                        &store.session_id,
                        Some(&work),
                        false,
                    ),
                )
                .unwrap();
        }
    }
    let view = engine.inspect(&turn).unwrap();
    assert!(
        view.nodes
            .iter()
            .filter(|(node, _)| !node.starts_with("speech/"))
            .map(|(_, state)| state)
            .all(|s| matches!(s, Disposition::Adopted | Disposition::Skipped)),
        "{:?}",
        view.nodes
    );
    assert_eq!(view.nodes["speech/lookup"], Disposition::Unrequested);
    assert_eq!(view.nodes["speech/synthesize"], Disposition::Waiting);
    let saved: String = store
        .connection
        .query_row("SELECT context FROM turns WHERE id=?1", [&turn], |r| {
            r.get(0)
        })
        .unwrap();
    let saved: Value = serde_json::from_str(&saved).unwrap();
    for field in [
        "translation",
        "userTranslation",
        "wordGloss",
        "userWordGloss",
        "skillAssessment",
        "skillAttribution",
        "reply_brief",
        "reply_assistance",
        "reply_explanations",
        "practiceObservation",
        "rewardEvents",
    ] {
        assert!(!saved[field].is_null(), "missing {field}");
    }
    assert_eq!(saved["speechSourceId"], "native-reply");
    assert!(
        saved["practiceObservation"]["credits"]
            .as_array()
            .unwrap()
            .iter()
            .all(|c| c["experience"] == 1 && c["effort"] == 0)
    );
    let assessment =
        crate::conversations::assessments::current(&store.connection, &turn, "skill_assessment")
            .unwrap()
            .unwrap();
    assert_eq!(
        saved["practiceObservation"]["inferenceAttemptId"],
        assessment.0
    );
    let feedback =
        crate::conversations::assessments::current(&store.connection, &turn, "coach_feedback")
            .unwrap()
            .unwrap();
    let mut decision = feedback.1["decision"].clone();
    decision["keptGoing"] = json!(true);
    let tx = store.connection.transaction().unwrap();
    crate::conversations::assessments::disclose(&tx, &turn, &feedback.0, &decision).unwrap();
    tx.commit().unwrap();
    assert_eq!(
        crate::conversations::assessments::context(&store.connection, &turn).unwrap()["coachDecision"]
            ["keptGoing"],
        true
    );
    assert_eq!(
        store
            .connection
            .query_row(
                "SELECT count(*) FROM operations WHERE turn_id=?1",
                [&turn],
                |r| r.get::<_, i64>(0)
            )
            .unwrap(),
        0
    );
    assert_eq!(
        store
            .connection
            .query_row("SELECT count(*) FROM message_assessments", [], |r| r
                .get::<_, i64>(0))
            .unwrap(),
        0
    );
    let checkpoint = graph_store::ReadStore::new(&mut store.connection, partition.clone())
        .unwrap()
        .checkpoint(graph_runtime::limits().checkpoint)
        .unwrap()
        .unwrap();
    drop(engine);
    let recovered = DurableEngine::recover(
        checkpoint,
        [graph.clone()],
        graph_runtime::limits(),
        &mut adapter(
            &mut store.connection,
            &partition,
            &turn,
            &store.config,
            &store.session_id,
            None,
            false,
        ),
    )
    .unwrap();
    assert_eq!(
        recovered.inspect(&turn).unwrap().nodes["assessment"],
        Disposition::Adopted
    );
    assert_eq!(
        crate::conversations::assessments::current(&store.connection, &turn, "skill_assessment")
            .unwrap()
            .unwrap(),
        assessment
    );
    let after: String = store
        .connection
        .query_row("SELECT context FROM turns WHERE id=?1", [&turn], |r| {
            r.get(0)
        })
        .unwrap();
    assert_eq!(
        serde_json::from_str::<Value>(&after).unwrap()["practiceObservation"],
        saved["practiceObservation"]
    );
    drop(recovered);
    verification::verify_product(&mut store, &conversation, &turn, graph);
}
