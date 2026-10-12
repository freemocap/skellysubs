use super::*;
use crate::ai::transport::provider::Completion;

pub(super) fn apply(store: &mut Store, action: Action) -> Receipt {
    store
        .execute(Command {
            session_id: store.session_id.clone(),
            action_id: id(),
            action,
        })
        .unwrap()
}

pub(super) fn coach_provider(store: &Store, replies: &[&str]) {
    use std::sync::{Arc, Mutex};
    let replies = Arc::new(Mutex::new(
        replies
            .iter()
            .map(|s| s.to_string())
            .collect::<std::collections::VecDeque<_>>(),
    ));
    store
        .graph_runtime
        .bind_provider(Arc::new(move |context, _| {
            let text = replies
                .lock()
                .unwrap()
                .pop_front()
                .expect("unexpected provider call");
            Box::pin(async move {
                context.observe(crate::ai::graph::ResponseEvidence {
                    usage: Some(crate::ai::graph::UsageEvidence {
                        input_tokens: Some(21),
                        output_tokens: Some(8),
                        total_tokens: None,
                        provenance: "test_provider".into(),
                    }),
                    ..Default::default()
                })?;
                Ok(text)
            })
        }));
}

pub(super) fn setup() -> (tempfile::TempDir, Store, String) {
    let dir = tempfile::tempdir().unwrap();
    let mut store = Store::open(&dir.path().join("test.sqlite3")).unwrap();
    // Lifecycle suites explicitly exercise automatic assistance. Default and
    // on-demand policies have their own activation tests.
    store.connection.execute("UPDATE learner SET preferences=json_set(preferences,'$.execution',json(?1))", [serde_json::json!({"assessment":"automatic","replyBrief":"automatic","reading":"automatic"}).to_string()]).unwrap();
    store
        .connection
        .execute(
            "UPDATE ai_config SET route='hosted',assessment_adapter='chat_model'",
            [],
        )
        .unwrap();
    store
        .set_hosted_connection(1, Some("test-credential"), "fixture@example.invalid")
        .unwrap();
    apply(
        &mut store,
        Action::CreateContact {
            language_id: "spanish".into(),
            details: crate::partners::persona::starter("spanish").unwrap(),
        },
    );
    let contact = store.snapshot().unwrap().contacts[0].id.clone();
    let conversation = apply(
        &mut store,
        Action::CreateConversation {
            contact_id: contact,
            title: "Test exchange".into(),
        },
    )
    .entity_id;
    store.connection.execute("UPDATE conversation_settings SET settings=json_set(settings,'$.readAloud',json('false')) WHERE conversation_id=?1", [&conversation]).unwrap();
    (dir, store, conversation)
}

pub(super) fn send(store: &Store, conversation: &str) -> Command {
    let revision = store
        .snapshot()
        .unwrap()
        .conversations
        .iter()
        .find(|c| c.id == conversation)
        .unwrap()
        .revision;
    Command {
        session_id: store.session_id.clone(),
        action_id: id(),
        action: Action::SendMessage {
            input: crate::learning::coaching::InputEvidence::default(),
            conversation_id: conversation.into(),
            text: "Hola, ¿cómo estás?".into(),
            expected_revision: revision,
        },
    }
}

pub(super) fn reply(text: &str) -> Completion {
    Completion {
        diagnostics: None,
        text: text.into(),
        finish_reason: "stop".into(),
        actual_model: "google/gemini-2.5-flash".into(),
        provider_id: "test-response".into(),
        input_tokens: Some(21),
        output_tokens: Some(8),
    }
}

pub(super) fn revision_command(
    store: &Store,
    conversation: &str,
    turn: &str,
    text: &str,
) -> Command {
    Command {
        session_id: store.session_id.clone(),
        action_id: id(),
        action: Action::ReviseTurn {
            conversation_id: conversation.into(),
            turn_id: turn.into(),
            text: text.into(),
            input: crate::learning::coaching::InputEvidence::default(),
            expected_revision: store
                .conversation_snapshot(conversation, None)
                .unwrap()
                .revision,
        },
    }
}

pub(super) fn wave2_context(store: &Store, turn: &str) -> serde_json::Value {
    crate::conversations::assessments::context(&store.connection, turn).unwrap()
}
