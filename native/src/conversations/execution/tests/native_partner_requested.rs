//! Explicit commands must reach adopted product fields, not merely Ready nodes.
use super::*;
use crate::{
    ai::{graph::*, transport::provider::Completion},
    learning::coaching::support_graph,
};
use serde_json::json;
use std::sync::Arc;

#[tokio::test]
async fn native_requested_reply_helpers_publish_without_activating_assessment() {
    let (_dir, mut store, conversation) = setup();
    store.connection.execute("UPDATE learner SET preferences=json_set(preferences,'$.execution.reading','on_demand','$.execution.replyBrief','on_demand','$.execution.assessment','on_demand')", []).unwrap();
    let turn = store
        .execute(send(&store, &conversation))
        .unwrap()
        .entity_id;
    store.graph_runtime.partner.bind(Arc::new(|invocation| Box::pin(async move {
        let operation = &invocation.identity().operation;
        let text = if *operation == prose::operation_contract() { "Hola.".into() }
        else if *operation == support_graph::Task::Brief.operation() { json!({"explanation":"A greeting."}).to_string() }
        else if *operation == support_graph::Task::Assistance.operation() {
            json!({"replies":[
                {"text":"Hola.","translation":"Hello.","romanization":"","pronunciation":"oh-lah"},
                {"text":"Buenos días.","translation":"Good morning.","romanization":"","pronunciation":"bweh-nos dee-ahs"}
            ],"frames":["Hola, ___.","Buenos días, ___."],"starters":["Hola", "Buenos días"]}).to_string()
        } else if *operation == support_graph::explanation::operation_contract() {
            json!({"cards":[{"quote":"Hola","title":"Greeting","body":"Hola is a greeting.","example":"Hola, Ana.","contrast":""}]}).to_string()
        } else { return Err(Fault{code:"fixture_optional_failure".into(),path:"provider".into()}); };
        Ok(Completion{text,finish_reason:"stop".into(),actual_model:"fixture".into(),provider_id:"fixture".into(),input_tokens:Some(1),output_tokens:Some(2),diagnostics:None})
    })), Arc::new(|_,_|Box::pin(async{Ok(None)})));
    for _ in 0..16 {
        native_coach::tick(&mut store).await;
    }
    let product = store.conversation_snapshot(&conversation, None).unwrap();
    let message = product
        .messages
        .iter()
        .find(|m| m.role == "assistant")
        .unwrap();
    assert!(message.reply_brief.is_none());
    let source = message.id.clone();
    for action in [
        Action::RequestMessageHelp {
            message_id: source.clone(),
            help: MessageHelp::ReplyBrief,
            retry: false,
        },
        Action::RequestSuggestions {
            message_id: source.clone(),
        },
        Action::RequestExplanations {
            message_id: source.clone(),
        },
    ] {
        assert!(apply(&mut store, action).entity_id.starts_with("graph:"));
    }
    for _ in 0..24 {
        native_coach::tick(&mut store).await;
    }
    let product = store.conversation_snapshot(&conversation, None).unwrap();
    let message = product.messages.iter().find(|m| m.id == source).unwrap();
    assert_eq!(
        message.reply_brief.as_ref().unwrap().explanation,
        "A greeting."
    );
    assert_eq!(message.brief_state.as_deref(), Some("succeeded"));
    assert_eq!(message.suggestions_state.as_deref(), Some("succeeded"));
    assert_eq!(message.explanations_state.as_deref(), Some("succeeded"));
    assert_eq!(
        message.reply_explanations.as_ref().unwrap().cards[0].quote,
        "Hola"
    );
    let view = store
        .graph_runtime
        .inspection(&store.connection, &conversation, &turn)
        .unwrap()
        .unwrap();
    assert_eq!(view.nodes["assessment"], Disposition::Unrequested);
    let learner = product
        .messages
        .iter()
        .find(|message| message.role == "user")
        .unwrap();
    assert!(learner.assessment_state.is_none());
    assert!(learner.assessment_error.is_none());

    assert_eq!(view.nodes["reply_gloss"], Disposition::Unrequested);
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
    assert_eq!(
        graph_runtime::outstanding(&store.connection, &store.graph_runtime).unwrap(),
        0
    );
}
