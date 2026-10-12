//! Real commands, native transport inputs, disclosure and one-time effort.
use super::*;
use crate::{
    ai::{graph::*, transport::provider::Completion},
    learning::coaching::feedback_graph,
};
use serde_json::json;
use std::sync::Arc;
type Reply = tokio::sync::oneshot::Sender<std::result::Result<Completion, Fault>>;
type Requests = tokio::sync::mpsc::UnboundedReceiver<(InvocationContext, Reply)>;
fn bind(store: &Store) -> Requests {
    let (send, receive) = tokio::sync::mpsc::unbounded_channel();
    store.graph_runtime.partner.bind(
        Arc::new(move |context| {
            let (answer, result) = tokio::sync::oneshot::channel();
            send.send((context, answer)).unwrap();
            Box::pin(async move { result.await.unwrap() })
        }),
        Arc::new(|_, _| Box::pin(async { Ok(None) })),
    );
    receive
}
fn command(store: &Store, action: Action) -> Command {
    Command {
        session_id: store.session_id.clone(),
        action_id: id(),
        action,
    }
}
async fn drain(
    store: &mut Store,
    requests: &mut Requests,
    note: Option<&str>,
    fail: bool,
) -> usize {
    let mut calls = 0;
    for _ in 0..32 {
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
        let task = tokio::spawn(claim.invocation.execute(EvidenceLimits {
            observations: 16,
            bytes: 65536,
        }));
        if claim.resource == Resource::Provider {
            let (context, answer) =
                tokio::time::timeout(std::time::Duration::from_secs(5), requests.recv())
                    .await
                    .unwrap()
                    .unwrap();
            let prepared = store
                .graph_runtime
                .authorize_text(&store.connection, context.identity())
                .unwrap();
            let feedback = context.identity().operation == feedback_graph::operation_contract();
            if feedback {
                let payload: serde_json::Value =
                    serde_json::from_str(&prepared.request.messages[1].content).unwrap();
                if let Some(note) = note {
                    assert_eq!(payload["learnerClarification"], note);
                }
                calls += 1;
            }
            context
                .observe(ResponseEvidence {
                    request_id: Some("feedback-fixture".into()),
                    ..Default::default()
                })
                .unwrap();
            let result = if feedback && fail {
                Err(Fault {
                    code: "fixture_provider_failure".into(),
                    path: "feedback".into(),
                })
            } else {
                Ok(Completion {
                    text: if feedback {
                        json!({"meaning_recovered":"full","items":[]}).to_string()
                    } else {
                        "Hola.".into()
                    },
                    finish_reason: "stop".into(),
                    actual_model: "fixture".into(),
                    provider_id: "feedback-fixture".into(),
                    input_tokens: Some(2),
                    output_tokens: Some(3),
                    diagnostics: None,
                })
            };
            answer.send(result).unwrap();
        }
        let report = task.await.unwrap();
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
    calls
}
#[tokio::test]
async fn native_feedback_context_publishes_fresh_receipts_and_preserves_disclosure_and_credit() {
    let (dir, mut store, conversation) = setup();
    store.connection.execute("UPDATE learner SET preferences=json_set(preferences,'$.execution.reading','on_demand','$.execution.assessment','on_demand','$.execution.replyBrief','on_demand')",[]).unwrap();
    let turn = store
        .execute(send(&store, &conversation))
        .unwrap()
        .entity_id;
    let mut requests = bind(&store);
    assert_eq!(drain(&mut store, &mut requests, None, false).await, 1);
    let product = store.conversation_snapshot(&conversation, None).unwrap();
    let source = product
        .messages
        .iter()
        .find(|m| m.role == "user")
        .unwrap()
        .id
        .clone();
    let original:String=store.connection.query_row("SELECT id FROM conversation_graph_assessments WHERE turn_id=?1 AND kind='coach_feedback'",[&turn],|r|r.get(0)).unwrap();
    let context = crate::conversations::assessments::context(&store.connection, &turn).unwrap();
    let tx = store.connection.unchecked_transaction().unwrap();
    crate::conversations::assessments::disclose(&tx, &turn, &original, &context["coachDecision"])
        .unwrap();
    tx.commit().unwrap();
    let note = "I meant yesterday, not today.";
    let request = command(
        &store,
        Action::ReassessFeedback {
            turn_id: turn.clone(),
            note: note.into(),
        },
    );
    store.execute(request.clone()).unwrap();
    store.execute(request).unwrap();
    let pending = store.conversation_snapshot(&conversation, None).unwrap();
    let message = pending.messages.iter().find(|m| m.id == source).unwrap();
    assert_eq!(message.feedback_context.as_deref(), Some(note));
    assert!(message.feedback.is_none());
    assert!(
        store
            .execute(command(
                &store,
                Action::ReassessFeedback {
                    turn_id: turn.clone(),
                    note: "Another note".into()
                }
            ))
            .is_err()
    );
    assert_eq!(drain(&mut store, &mut requests, Some(note), false).await, 1);
    assert_eq!(
        crate::learning::effort::read(&store.connection, "spanish")
            .unwrap()
            .explorations,
        1
    );
    assert_eq!(
        store
            .connection
            .query_row(
                "SELECT count(*) FROM conversation_graph_assessments WHERE turn_id=?1",
                [&turn],
                |r| r.get::<_, i64>(0)
            )
            .unwrap(),
        2
    );
    assert_eq!(
        store
            .connection
            .query_row(
                "SELECT count(*) FROM conversation_graph_disclosures",
                [],
                |r| r.get::<_, i64>(0)
            )
            .unwrap(),
        1
    );
    store
        .execute(command(
            &store,
            Action::ReassessFeedback {
                turn_id: turn.clone(),
                note: note.into(),
            },
        ))
        .unwrap();
    assert_eq!(drain(&mut store, &mut requests, Some(note), true).await, 1);
    let failed = store.conversation_snapshot(&conversation, None).unwrap();
    let failed = failed.messages.iter().find(|m| m.id == source).unwrap();
    assert_eq!(failed.feedback_state.as_deref(), Some("failed"));
    assert!(
        failed
            .feedback_error
            .as_ref()
            .unwrap()
            .contains("feedback-fixture")
    );
    store
        .execute(command(
            &store,
            Action::RequestMessageHelp {
                message_id: source.clone(),
                help: MessageHelp::Coaching,
                retry: true,
            },
        ))
        .unwrap();
    assert_eq!(drain(&mut store, &mut requests, Some(note), false).await, 1);
    assert_eq!(
        crate::learning::effort::read(&store.connection, "spanish")
            .unwrap()
            .explorations,
        1
    );
    assert_eq!(
        store
            .connection
            .query_row(
                "SELECT count(*) FROM sqlite_master WHERE name='inference_executions'",
                [],
                |r| r.get::<_, i64>(0)
            )
            .unwrap(),
        0
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
    drop(store);
    let mut store = Store::open(&dir.path().join("test.sqlite3")).unwrap();
    let final_view = store.conversation_snapshot(&conversation, None).unwrap();
    let message = final_view.messages.iter().find(|m| m.id == source).unwrap();
    assert_eq!(message.feedback_state.as_deref(), Some("succeeded"));
    assert!(message.feedback.is_some());
    assert_eq!(message.feedback_context.as_deref(), Some(note));
    for invalid in [" ".to_string(), "bad\0note".into(), "x".repeat(2001)] {
        assert!(
            store
                .execute(command(
                    &store,
                    Action::ReassessFeedback {
                        turn_id: turn.clone(),
                        note: invalid,
                    }
                ))
                .is_err()
        );
    }
    store
        .execute(command(
            &store,
            Action::ReassessFeedback {
                turn_id: turn.clone(),
                note: "Pending when source changes".into(),
            },
        ))
        .unwrap();
    let revision = store
        .snapshot()
        .unwrap()
        .conversations
        .iter()
        .find(|c| c.id == conversation)
        .unwrap()
        .revision;
    store
        .execute(command(
            &store,
            Action::ReviseTurn {
                conversation_id: conversation.clone(),
                turn_id: turn.clone(),
                text: "Replacement message.".into(),
                input: crate::learning::coaching::InputEvidence::default(),
                expected_revision: revision,
            },
        ))
        .unwrap();
    assert!(
        store
            .execute(command(
                &store,
                Action::ReassessFeedback {
                    turn_id: turn.clone(),
                    note: "Old source".into(),
                }
            ))
            .is_err()
    );
    assert!(
        store
            .execute(command(
                &store,
                Action::RequestMessageHelp {
                    message_id: source,
                    help: MessageHelp::Coaching,
                    retry: true,
                }
            ))
            .is_err()
    );
}

#[tokio::test]
async fn live_feedback_view_matches_history_and_refuses_damaged_current_records() {
    let (_dir, mut store, conversation) = setup();
    store.connection.execute("UPDATE learner SET preferences=json_set(preferences,'$.execution.reading','on_demand','$.execution.assessment','on_demand','$.execution.replyBrief','on_demand')",[]).unwrap();
    let turn = store
        .execute(send(&store, &conversation))
        .unwrap()
        .entity_id;
    let mut requests = bind(&store);
    drain(&mut store, &mut requests, None, false).await;
    let expected = (
        crate::conversations::assessments::context(&store.connection, &turn).unwrap(),
        crate::conversations::assessments::feedback(&store.connection, &turn).unwrap(),
    );
    let actual =
        crate::conversations::assessments::view(&store.connection, &turn, &store.graph_runtime)
            .unwrap();
    assert_eq!(
        serde_json::to_value(actual).unwrap(),
        serde_json::to_value(expected).unwrap()
    );
    store.conversation_snapshot(&conversation, None).unwrap();

    // The running host must verify persisted records, not silently fall back
    // to replaying an intact checkpoint when the current record read fails.
    let tx = store.connection.unchecked_transaction().unwrap();
    assert!(tx.execute(
        "UPDATE graph_records SET payload=x'7B7D' WHERE engine_id=(SELECT engine_id FROM turn_execution_owners WHERE turn_id=?1)",
        [&turn],
    ).unwrap() > 0);
    assert!(crate::conversations::assessments::view(&tx, &turn, &store.graph_runtime).is_err());
    tx.rollback().unwrap();
    store.conversation_snapshot(&conversation, None).unwrap();
}
