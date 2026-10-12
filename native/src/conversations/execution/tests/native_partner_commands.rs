//! Production command admission, opening publication, restart and subsequent Send.
use super::*;
use crate::ai::graph::*;
use std::sync::Arc;

fn start(store: &Store, conversation: &str, learner: bool) -> Command {
    let snapshot = store.snapshot().unwrap();
    let settings = &snapshot
        .conversations
        .iter()
        .find(|c| c.id == conversation)
        .unwrap()
        .settings;
    Command {
        session_id: store.session_id.clone(),
        action_id: id(),
        action: Action::StartConversation {
            conversation_id: conversation.into(),
            configuration: crate::conversations::direction::ConversationStartConfig {
                prompt_editor: None,
                difficulty: settings.difficulty.clone(),
                variety_id: settings.variety_id.clone(),
                direction: settings.direction.clone(),
            },
            message: learner.then(|| "Hola, ¿cómo estás?".into()),
            input: learner.then(Default::default),
            expected_revision: snapshot.revision,
        },
    }
}

#[tokio::test]
async fn partner_start_and_next_send_publish_through_command_owned_graphs() {
    for learner in [false, true] {
        let (dir, mut store, conversation) = setup();
        let command = start(&store, &conversation, learner);
        let turn = store.execute(command.clone()).unwrap().entity_id;
        assert_eq!(store.execute(command).unwrap().entity_id, turn);
        store.graph_runtime.partner.bind(
            Arc::new(|invocation| {
                Box::pin(async move {
                    if invocation.identity().operation != prose::operation_contract() {
                        return Err(Fault {
                            code: "fixture_optional_failure".into(),
                            path: "provider".into(),
                        });
                    }
                    Ok(reply("Hola."))
                })
            }),
            Arc::new(|_, _| Box::pin(async { Ok(None) })),
        );
        for _ in 0..40 {
            let claim = store
                .graph_runtime
                .next(
                    &mut store.connection,
                    true,
                    &store.config,
                    &store.session_id,
                )
                .unwrap();
            let Some(claim) = claim else { continue };
            let report = claim
                .invocation
                .execute(EvidenceLimits {
                    observations: 16,
                    bytes: 65536,
                })
                .await;
            store
                .graph_runtime
                .finish(&mut store.connection, &conversation, &turn, report)
                .unwrap();
        }
        let product = store.conversation_snapshot(&conversation, None).unwrap();
        assert!(
            product
                .messages
                .iter()
                .any(|m| m.role == "assistant" && m.text == "Hola.")
        );
        assert_eq!(
            store
                .connection
                .query_row("SELECT state FROM turns WHERE id=?1", [&turn], |r| r
                    .get::<_, String>(0))
                .unwrap(),
            "succeeded"
        );
        let inspection = store
            .graph_runtime
            .inspection(&store.connection, &conversation, &turn)
            .unwrap()
            .unwrap();
        assert_eq!(inspection.nodes["reply"], Disposition::Adopted);
        assert_eq!(
            store
                .connection
                .query_row("SELECT count(*) FROM operations", [], |r| r
                    .get::<_, i64>(0))
                .unwrap(),
            0
        );
        assert_eq!(
            store
                .connection
                .query_row("SELECT count(*) FROM attempts", [], |r| r.get::<_, i64>(0))
                .unwrap(),
            0
        );
        if learner {
            let usage = store.profile().unwrap().global;
            assert_eq!(usage.learner_messages, 1);
            assert_eq!(usage.persona_messages, 1);
        }
        let message = product
            .messages
            .iter()
            .find(|m| m.role == "assistant")
            .unwrap();
        assert_eq!(
            crate::conversations::phrase_start::source(&store.connection, &message.id, "Hola")
                .unwrap(),
            conversation
        );
        drop(store);
        let mut store = Store::open(&dir.path().join("test.sqlite3")).unwrap();
        let next = store
            .execute(send(&store, &conversation))
            .unwrap()
            .entity_id;
        let raw: String = store
            .connection
            .query_row("SELECT context FROM turns WHERE id=?1", [&next], |r| {
                r.get(0)
            })
            .unwrap();
        let captured: serde_json::Value = serde_json::from_str(&raw).unwrap();
        assert!(
            captured["messages"]
                .as_array()
                .unwrap()
                .iter()
                .any(|m| m["role"] == "assistant" && m["content"] == "Hola.")
        );
        assert_eq!(
            store
                .connection
                .query_row(
                    "SELECT count(*) FROM turn_execution_owners WHERE executor='graph'",
                    [],
                    |r| r.get::<_, i64>(0)
                )
                .unwrap(),
            2
        );
    }
}

#[test]
fn native_partner_admission_failure_rolls_back_product_and_receipt() {
    let (_dir, mut store, conversation) = setup();
    let command = start(&store, &conversation, false);
    store.connection.execute_batch("CREATE TEMP TRIGGER reject_partner BEFORE INSERT ON graph_engines BEGIN SELECT RAISE(ABORT,'fixture admission failure'); END;").unwrap();
    assert!(store.execute(command.clone()).is_err());
    for table in [
        "turns",
        "operations",
        "turn_execution_owners",
        "conversation_openings",
    ] {
        assert_eq!(
            store
                .connection
                .query_row(&format!("SELECT count(*) FROM {table}"), [], |r| r
                    .get::<_, i64>(0))
                .unwrap(),
            0
        );
    }
    assert_eq!(
        store
            .connection
            .query_row(
                "SELECT count(*) FROM receipts WHERE action_id=?1",
                [&command.action_id],
                |r| r.get::<_, i64>(0)
            )
            .unwrap(),
        0
    );
    store
        .connection
        .execute_batch("DROP TRIGGER reject_partner")
        .unwrap();
    store.execute(command).unwrap();
}

#[tokio::test]
async fn seeded_openings_preserve_exact_unicode_and_expose_rejected_replies() {
    for guide in [false, true] {
        let (_dir, mut store, source) = setup();
        let (action, expected) = if guide {
            let settings = store.snapshot().unwrap().conversations[0].settings.clone();
            let context = store
                .config
                .guide_source("spanish", "time_events")
                .unwrap()
                .context(&settings.variety_id);
            let phrase = context.examples[0].clone();
            (
                Action::StartGuideConversation {
                    source_conversation_id: source.clone(),
                    guide: context.reference,
                    example: 0,
                    phrase: None,
                    expected_revision: store.snapshot().unwrap().revision,
                },
                phrase,
            )
        } else {
            store.execute(send(&store, &source)).unwrap();
            let source_replies =
                Arc::new(std::sync::Mutex::new(std::collections::VecDeque::from([
                    "Un cafe\u{301}, por favor.".to_string(),
                    "Wrong phrase.".to_string(),
                    "cafe\u{301} ¿Y tú?".to_string(),
                ])));
            store.graph_runtime.partner.bind(
                Arc::new(move |invocation| {
                    let source_replies = source_replies.clone();
                    Box::pin(async move {
                        if invocation.identity().operation != prose::operation_contract() {
                            return Err(Fault {
                                code: "fixture_optional_failure".into(),
                                path: "provider".into(),
                            });
                        }
                        Ok(reply(
                            &source_replies
                                .lock()
                                .unwrap()
                                .pop_front()
                                .expect("unexpected reply"),
                        ))
                    })
                }),
                Arc::new(|_, _| Box::pin(async { Ok(None) })),
            );
            drive(&mut store).await;
            let message = store.conversation_snapshot(&source, None).unwrap().messages[1]
                .id
                .clone();
            (
                Action::StartPhraseConversation {
                    source_message_id: message,
                    phrase: "cafe\u{301}".into(),
                    contact_id: store.snapshot().unwrap().conversations[0]
                        .contact_id
                        .clone(),
                    expected_revision: store.snapshot().unwrap().revision,
                },
                "cafe\u{301}".to_string(),
            )
        };
        let command = Command {
            session_id: store.session_id.clone(),
            action_id: id(),
            action,
        };
        let conversation = store.execute(command).unwrap().entity_id;
        let turn = store
            .conversation_snapshot(&conversation, None)
            .unwrap()
            .turns[0]
            .id
            .clone();
        let replies = Arc::new(std::sync::Mutex::new(std::collections::VecDeque::from([
            "Wrong phrase.".to_string(),
            format!("{expected} ¿Y tú?"),
        ])));
        store.graph_runtime.partner.bind(
            Arc::new(move |invocation| {
                let replies = replies.clone();
                Box::pin(async move {
                    if invocation.identity().operation != prose::operation_contract() {
                        return Err(Fault {
                            code: "fixture_optional_failure".into(),
                            path: "provider".into(),
                        });
                    }
                    Ok(reply(
                        &replies
                            .lock()
                            .unwrap()
                            .pop_front()
                            .expect("unrequested retry"),
                    ))
                })
            }),
            Arc::new(|_, _| Box::pin(async { Ok(None) })),
        );
        drive(&mut store).await;
        let product = store.conversation_snapshot(&conversation, None).unwrap();
        assert!(product.messages.is_empty());
        assert_eq!(
            store
                .graph_runtime
                .inspection(&store.connection, &conversation, &turn)
                .unwrap()
                .unwrap()
                .nodes["reply"],
            Disposition::Failed
        );
        store
            .execute(Command {
                session_id: store.session_id.clone(),
                action_id: id(),
                action: Action::ControlTurn {
                    turn_id: turn.clone(),
                    control: TurnControl::Retry,
                },
            })
            .unwrap();
        drive(&mut store).await;
        let product = store.conversation_snapshot(&conversation, None).unwrap();
        assert_eq!(product.messages.len(), 1);
        assert!(product.messages[0].text.contains(&expected));
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
    }
}

async fn drive(store: &mut Store) {
    for _ in 0..40 {
        let Some(claim) = store
            .graph_runtime
            .next(
                &mut store.connection,
                true,
                &store.config,
                &store.session_id,
            )
            .unwrap()
        else {
            continue;
        };
        let report = claim
            .invocation
            .execute(EvidenceLimits {
                observations: 16,
                bytes: 65536,
            })
            .await;
        store
            .graph_runtime
            .finish(
                &mut store.connection,
                &claim.conversation,
                &claim.run,
                report,
            )
            .unwrap();
    }
}

#[tokio::test]
async fn skill_opening_uses_native_admission_and_preserves_captured_focus() {
    let (_dir, mut store, source) = setup();
    let snapshot = store.snapshot().unwrap();
    let action = Action::StartSkillConversation {
        source_conversation_id: source,
        language: "spanish".into(),
        variety: snapshot.conversations[0].settings.variety_id.clone(),
        skill_id: "time_events".into(),
        subskill_id: Some("past_events".into()),
        expected_revision: snapshot.revision,
    };
    let conversation = store
        .execute(Command {
            session_id: store.session_id.clone(),
            action_id: id(),
            action,
        })
        .unwrap()
        .entity_id;
    let turn = store
        .conversation_snapshot(&conversation, None)
        .unwrap()
        .turns[0]
        .id
        .clone();
    assert_eq!(
        wave2_context(&store, &turn)["practiceFocus"]["subskillId"],
        "past_events"
    );
    store.graph_runtime.partner.bind(
        Arc::new(|invocation| {
            Box::pin(async move {
                if invocation.identity().operation == prose::operation_contract() {
                    Ok(reply("¿Qué hiciste ayer?"))
                } else {
                    Err(Fault {
                        code: "fixture_optional_failure".into(),
                        path: "provider".into(),
                    })
                }
            })
        }),
        Arc::new(|_, _| Box::pin(async { Ok(None) })),
    );
    drive(&mut store).await;
    assert_eq!(
        store
            .conversation_snapshot(&conversation, None)
            .unwrap()
            .messages[0]
            .text,
        "¿Qué hiciste ayer?"
    );
    assert_eq!(
        store
            .connection
            .query_row("SELECT count(*) FROM operations", [], |r| r
                .get::<_, i64>(0))
            .unwrap(),
        0
    );
}
