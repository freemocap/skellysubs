//! Exercise the real RetryGloss command against adopted native results.
use super::*;
use crate::{
    ai::{graph::*, transport::provider::Completion},
    language::gloss_graph,
};
use serde_json::json;
use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};

#[tokio::test]
async fn fresh_partial_gloss_reuses_operation_preserves_spans_and_rejects_replaced_source() {
    let (dir, mut store, conversation) = setup();
    store.connection.execute("UPDATE learner SET preferences=json_set(preferences,'$.execution.reading','on_demand','$.execution.assessment','on_demand','$.execution.replyBrief','on_demand')",[]).unwrap();
    let turn = store
        .execute(send(&store, &conversation))
        .unwrap()
        .entity_id;
    let count = Arc::new(AtomicUsize::new(0));
    let calls = count.clone();
    store.graph_runtime.partner.bind(Arc::new(move |invocation| {
        let calls=calls.clone();
        Box::pin(async move {
            let text=if invocation.identity().operation==prose::operation_contract() {"Hola mundo.".into()}
            else if invocation.identity().operation==gloss_graph::operation_contract() {
                if calls.fetch_add(1,Ordering::SeqCst)==0 {json!({"spans":[{"first":"g0000","last":"g0003","kind":"gloss","gloss":"hello","pronunciation":"oh-lah"}]}).to_string()} else {json!({"spans":[]}).to_string()}
            } else {return Err(Fault{code:"unexpected_operation".into(),path:"fixture".into()});};
            Ok(Completion{text,finish_reason:"stop".into(),actual_model:"fixture".into(),provider_id:"gloss-fixture".into(),input_tokens:Some(2),output_tokens:Some(3),diagnostics:None})
        })
    }),Arc::new(|_,_|Box::pin(async{Ok(None)})));
    for _ in 0..16 {
        native_coach::tick(&mut store).await;
    }
    let source = store
        .conversation_snapshot(&conversation, None)
        .unwrap()
        .messages
        .iter()
        .find(|m| m.role == "assistant")
        .unwrap()
        .id
        .clone();
    store
        .execute(Command {
            session_id: store.session_id.clone(),
            action_id: id(),
            action: Action::RequestMessageHelp {
                message_id: source.clone(),
                help: MessageHelp::WordGloss,
                retry: false,
            },
        })
        .unwrap();
    for _ in 0..12 {
        native_coach::tick(&mut store).await;
    }
    let before = store.conversation_snapshot(&conversation, None).unwrap();
    let message = before.messages.iter().find(|m| m.id == source).unwrap();
    assert_eq!(
        message.word_gloss.as_ref().unwrap().coverage,
        GlossCoverage::Partial
    );
    assert!(
        message
            .word_gloss
            .as_ref()
            .unwrap()
            .segments
            .iter()
            .any(|s| s.gloss.as_deref() == Some("hello"))
    );
    let original = message.gloss_operation_id.clone().unwrap();
    let command = Command {
        session_id: store.session_id.clone(),
        action_id: id(),
        action: Action::RetryGloss {
            operation_id: original.clone(),
        },
    };
    store.connection.execute_batch("CREATE TEMP TRIGGER reject_helper BEFORE INSERT ON graph_helper_requests BEGIN SELECT RAISE(ABORT,'fixture rejection'); END;").unwrap();
    assert!(store.execute(command.clone()).is_err());
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
        .execute_batch("DROP TRIGGER reject_helper")
        .unwrap();
    let receipt = store.execute(command.clone()).unwrap();
    assert!(receipt.entity_id.starts_with("graph-helper:"));
    assert_eq!(store.execute(command).unwrap().entity_id, receipt.entity_id);
    assert_eq!(
        store
            .connection
            .query_row("SELECT count(*) FROM graph_helper_requests", [], |r| r
                .get::<_, i64>(0))
            .unwrap(),
        1
    );
    for _ in 0..12 {
        native_coach::tick(&mut store).await;
    }
    let after = store.conversation_snapshot(&conversation, None).unwrap();
    let repaired = after.messages.iter().find(|m| m.id == source).unwrap();
    assert_eq!(repaired.gloss_state.as_deref(), Some("succeeded"));
    assert_ne!(repaired.gloss_operation_id.as_ref(), Some(&original));
    assert!(
        repaired
            .word_gloss
            .as_ref()
            .unwrap()
            .segments
            .iter()
            .any(|s| s.gloss.as_deref() == Some("hello"))
    );
    assert_eq!(count.load(Ordering::SeqCst), 2);
    let latest = repaired.gloss_operation_id.clone().unwrap();
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
            .query_row(
                "SELECT count(*) FROM sqlite_master WHERE name='inference_executions'",
                [],
                |r| r.get::<_, i64>(0)
            )
            .unwrap(),
        0
    );
    drop(store);
    let mut store = Store::open(&dir.path().join("test.sqlite3")).unwrap();
    let reopened = store.conversation_snapshot(&conversation, None).unwrap();
    assert_eq!(
        reopened
            .messages
            .iter()
            .find(|m| m.id == source)
            .unwrap()
            .gloss_operation_id
            .as_deref(),
        Some(latest.as_str())
    );
    let revision = store
        .snapshot()
        .unwrap()
        .conversations
        .iter()
        .find(|c| c.id == conversation)
        .unwrap()
        .revision;
    store
        .execute(Command {
            session_id: store.session_id.clone(),
            action_id: id(),
            action: Action::ReviseTurn {
                conversation_id: conversation.clone(),
                turn_id: turn.clone(),
                text: "Replacement message.".into(),
                input: crate::learning::coaching::InputEvidence::default(),
                expected_revision: revision,
            },
        })
        .unwrap();
    assert!(
        store
            .execute(Command {
                session_id: store.session_id.clone(),
                action_id: id(),
                action: Action::RetryGloss {
                    operation_id: latest
                }
            })
            .is_err()
    );
}
