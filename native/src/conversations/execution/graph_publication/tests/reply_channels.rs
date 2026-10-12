//! Publication integration only: access/transport capability checks are covered
//! by the host suites. These cases drive the real native claim/settle/adopt path.
use super::*;

#[tokio::test]
async fn admitted_reply_identity_is_the_published_message_identity() {
    use crate::conversations::execution::{context, reply_reading_graph};
    use crate::language::{source_graph::SourceText, translation_graph::Languages};
    use std::sync::Arc;
    let graph = Arc::new(
        reply_reading_graph::compile(
            context::Kind::Reply,
            Arc::new(|_, _| Box::pin(async { Ok("Réponse 日本語".into()) })),
            Arc::new(|_, _| Box::pin(async { panic!("disabled translation") })),
            Arc::new(|_, _| Box::pin(async { panic!("disabled gloss") })),
        )
        .unwrap(),
    );
    let mut db = db();
    let target = crate::ai::connections::access::ResolvedTarget {
        audio_resolution: None,
        route: crate::model::ConnectionRoute::Hosted,
        revision: 1,
        url: format!("{}/v1/operations", crate::ai::hosted::ORIGIN),
        model: "fixture".into(),
        credential: Some("reference".into()),
    };
    let capture = reply_reading_graph::capture(reply_reading_graph::Captured {
        context: context::Captured {
            kind: context::Kind::Reply,
            messages: vec![
                crate::ai::transport::provider::PromptMessage {
                    role: "system".into(),
                    content: "Instructions".into(),
                },
                crate::ai::transport::provider::PromptMessage {
                    role: "user".into(),
                    content: "Question".into(),
                },
            ],
            source_ids: vec!["user".into()],
        },
        learner_source: Some(SourceText {
            id: "user".into(),
            text: "Question".into(),
        }),
        reply_source_id: "reserved-reply".into(),
        languages: Languages {
            source: "english".into(),
            destination: "english".into(),
            destination_writing: vec![],
        },
        gloss_settings: crate::language::linguistics::adapter::settings::Settings {
            target_language: "english".into(),
            explanation_language: "english".into(),
            explanation_writing: vec![],
            romanization: vec![],
            segmentation: vec![],
            romanization_enabled: false,
        },
        gloss_target: target.clone(),
        reply_target: target.clone(),
        reading_target: target,
        install_id: "fixture-install".into(),
    })
    .unwrap();
    let partition = Partition {
        owner: crate::ai::graph_store::Owner::Conversation("conversation".into()),
        catalog: catalog_id([graph.identity()]).unwrap(),
    };
    let policy = graph
        .artifact()
        .definition
        .nodes
        .keys()
        .map(|name| {
            (
                name.clone(),
                if ["context", "reply", "reply_source"].contains(&name.as_str()) {
                    Activation::Automatic
                } else {
                    Activation::Disabled
                },
            )
        })
        .collect();
    let mut engine = DurableEngine::create_with_run(
        [graph.clone()],
        limits(),
        Event::Begin {
            run: "run".into(),
            artifact: graph.identity().into(),
            inputs: capture,
            scope: "captured".into(),
            policy,
        },
        &mut adapter(&mut db, &partition, "persona_reply", false),
    )
    .unwrap();
    assert_eq!(
        db.query_row(
            "SELECT message_id FROM conversation_graph_reply_sources",
            [],
            |r| r.get::<_, String>(0)
        )
        .unwrap(),
        "reserved-reply"
    );
    assert_eq!(count(&db, "messages"), 1);
    for node in ["context", "reply", "reply_source"] {
        engine
            .apply(
                Event::Advance(Capacity {
                    local: 1,
                    provider: 1,
                }),
                &mut adapter(&mut db, &partition, "persona_reply", false),
            )
            .unwrap();
        let attempt = engine.inspect("run").unwrap().attempts[node]
            .last()
            .unwrap()
            .id;
        let report = engine
            .claim(
                "run",
                node,
                attempt,
                &mut adapter(&mut db, &partition, "persona_reply", false),
            )
            .unwrap()
            .execute(EvidenceLimits {
                observations: 10,
                bytes: 8192,
            })
            .await;
        engine
            .apply(
                Event::SettleObserved(report),
                &mut adapter(&mut db, &partition, "persona_reply", false),
            )
            .unwrap();
        if node == "reply" {
            assert!(
                engine
                    .adopt(
                        "run",
                        node,
                        attempt,
                        &mut adapter(&mut db, &partition, "persona_reply", true)
                    )
                    .is_err()
            );
            assert_eq!(count(&db, "messages"), 1);
            assert_eq!(count(&db, "conversation_graph_reply_sources"), 1);
        }
        engine
            .adopt(
                "run",
                node,
                attempt,
                &mut adapter(&mut db, &partition, "persona_reply", false),
            )
            .unwrap();
    }
    let message: (String, String) = db
        .query_row(
            "SELECT id,text FROM messages WHERE role='assistant'",
            [],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .unwrap();
    assert_eq!(message, ("reserved-reply".into(), "Réponse 日本語".into()));
    assert_eq!(count(&db, "operations"), 0);
    assert_eq!(count(&db, "effort_awards"), 0);
}

#[test]
fn reply_effect_declarations_reject_missing_nodes_ports_and_non_text_contracts() {
    for (node, port) in [
        ("missing", "text"),
        ("reply", "missing"),
        ("context", "context"),
    ] {
        let mut db = db();
        let graph = executable();
        let partition = Partition {
            owner: crate::ai::graph_store::Owner::Conversation("conversation".into()),
            catalog: catalog_id([graph.identity()]).unwrap(),
        };
        let mut adapter = TransactionStore::new(
            db.transaction().unwrap(),
            partition,
            100_000,
            |db: &Connection, request: &CommitRequest<'_>| {
                declare_reply(db, request, node, port)
                    .map(|_| ())
                    .map_err(|cause| {
                        assert_eq!(cause.code, ErrorCode::Validation);
                        assert!(cause.message.contains("generated-text output"));
                        Fault {
                            code: "bad_reply_binding".into(),
                            path: "effect".into(),
                        }
                    })
            },
        );
        assert!(
            DurableEngine::create_with_run(
                [graph.clone()],
                limits(),
                Event::Begin {
                    run: "run".into(),
                    artifact: graph.identity().into(),
                    inputs: inputs("Reply"),
                    scope: "captured".into(),
                    policy: Default::default(),
                },
                &mut adapter
            )
            .is_err()
        );
        drop(adapter);
        assert_eq!(count(&db, "graph_engines"), 0);
        assert_eq!(count(&db, "conversation_graph_effects"), 0);
    }
}

fn adapter<'a>(
    db: &'a mut Connection,
    partition: &Partition,
    channel: &'static str,
    deny: bool,
) -> TransactionStore<'a, impl FnMut(&Connection, &CommitRequest<'_>) -> graph::Result<()> + 'a> {
    TransactionStore::new(
        db.transaction().unwrap(),
        partition.clone(),
        100_000,
        move |db: &Connection, request: &CommitRequest<'_>| {
            let outcome = (|| -> crate::model::Result<()> {
                match &request.intent {
                    CommitIntent::Begin { authority, .. } => {
                        db.execute(
                            "INSERT INTO turn_execution_owners VALUES('turn','graph',?1,?2,?3,?4)",
                            params![
                                channel,
                                request.next.stamp().engine,
                                authority.run,
                                authority.artifact
                            ],
                        )?;
                        declare_reply(db, request, "reply", "text")?;
                    }
                    CommitIntent::Adopt { authority, .. } if authority.node == Some("reply") => {
                        publish_reply(db, request, |_, _| {
                            if deny {
                                Err(AppError::new(ErrorCode::Conflict, "revoked"))
                            } else {
                                Ok(())
                            }
                        })?;
                    }
                    CommitIntent::Dispatch { work, .. } if work.resource == Resource::Provider => {
                        crate::conversations::execution::graph_authority::dispatch(db, request)?;
                    }
                    _ => (),
                }
                Ok(())
            })();
            outcome.map_err(|_| Fault {
                code: "publication_rejected".into(),
                path: "owner".into(),
            })
        },
    )
}

#[tokio::test]
async fn all_reply_channels_publish_once_with_channel_specific_credit_and_atomic_rejection() {
    for channel in ["coach", "persona_reply", "persona_opening"] {
        let mut db = db();
        let graph = executable();
        let partition = Partition {
            owner: crate::ai::graph_store::Owner::Conversation("conversation".into()),
            catalog: catalog_id([graph.identity()]).unwrap(),
        };
        let mut engine = DurableEngine::create_with_run(
            [graph.clone()],
            limits(),
            Event::Begin {
                run: "run".into(),
                artifact: graph.identity().into(),
                inputs: inputs("Réponse 日本語"),
                scope: "captured".into(),
                policy: Default::default(),
            },
            &mut adapter(&mut db, &partition, channel, false),
        )
        .unwrap();
        for node in ["context", "reply"] {
            engine
                .apply(
                    Event::Advance(Capacity {
                        local: 1,
                        provider: 1,
                    }),
                    &mut adapter(&mut db, &partition, channel, false),
                )
                .unwrap();
            let attempt = engine.inspect("run").unwrap().attempts[node]
                .last()
                .unwrap()
                .id;
            let report = engine
                .claim(
                    "run",
                    node,
                    attempt,
                    &mut adapter(&mut db, &partition, channel, false),
                )
                .unwrap()
                .execute(EvidenceLimits {
                    observations: 10,
                    bytes: 8192,
                })
                .await;
            engine
                .apply(
                    Event::SettleObserved(report),
                    &mut adapter(&mut db, &partition, channel, false),
                )
                .unwrap();
            if node == "reply" {
                assert!(
                    engine
                        .adopt(
                            "run",
                            node,
                            attempt,
                            &mut adapter(&mut db, &partition, channel, true)
                        )
                        .is_err()
                );
                assert_eq!(count(&db, "conversation_graph_publications"), 0);
                assert_eq!(count(&db, "messages"), 1);
                assert_eq!(count(&db, "effort_awards"), 0);
                let context: String = db
                    .query_row("SELECT context FROM turns WHERE id='turn'", [], |r| {
                        r.get(0)
                    })
                    .unwrap();
                let context: serde_json::Value = serde_json::from_str(&context).unwrap();
                assert!(context.get("speechSourceId").is_none());
                assert!(context.get("speechSourceText").is_none());
            }
            engine
                .adopt(
                    "run",
                    node,
                    attempt,
                    &mut adapter(&mut db, &partition, channel, false),
                )
                .unwrap();
            assert!(
                engine
                    .adopt(
                        "run",
                        node,
                        attempt,
                        &mut adapter(&mut db, &partition, channel, false)
                    )
                    .is_err()
            );
        }
        assert_eq!(count(&db, "conversation_graph_publications"), 1);
        assert_eq!(count(&db, "messages"), 2);
        assert_eq!(count(&db, "operations"), 0);
        assert_eq!(count(&db, "effort_awards"), i64::from(channel == "coach"));
        let role: String = db
            .query_row("SELECT role FROM conversation_graph_effects", [], |r| {
                r.get(0)
            })
            .unwrap();
        assert_eq!(
            role,
            if channel == "coach" {
                "coach_reply"
            } else {
                channel
            }
        );
        let text: String = db
            .query_row(
                "SELECT text FROM messages WHERE role='assistant'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(text, "Réponse 日本語");
        let context: String = db
            .query_row("SELECT context FROM turns WHERE id='turn'", [], |r| {
                r.get(0)
            })
            .unwrap();
        let context: serde_json::Value = serde_json::from_str(&context).unwrap();
        if channel == "coach" {
            assert!(context.get("speechSourceId").is_none());
            assert!(context.get("speechSourceText").is_none());
        } else {
            let message: String = db
                .query_row(
                    "SELECT message_id FROM conversation_graph_publications",
                    [],
                    |r| r.get(0),
                )
                .unwrap();
            assert_eq!(context["speechSourceId"], message);
            assert_eq!(context["speechSourceText"], text);
        }
    }
}
