use super::*;
use crate::model::Action;
use tauri::ipc::{InvokeResponseBody, IpcResponse};

fn apply(store: &mut Store, action: Action) -> Receipt {
    store
        .execute(Command {
            session_id: store.session_id.clone(),
            action_id: uuid::Uuid::new_v4().to_string(),
            action,
        })
        .unwrap()
}

fn fixture() -> (tempfile::TempDir, Arc<Application>, String) {
    let directory = tempfile::tempdir().unwrap();
    let state = Application::recording_fixture(&directory.path().join("workspace.sqlite3"));
    let conversation = {
        let mut store = state.lock().unwrap();
        apply(
            &mut store,
            Action::CreateContact {
                language_id: "spanish".into(),
                details: crate::partners::persona::starter("spanish").unwrap(),
            },
        );
        let contact = store.snapshot().unwrap().contacts[0].id.clone();
        apply(
            &mut store,
            Action::CreateConversation {
                contact_id: contact,
                title: "Cached chat".into(),
            },
        )
        .entity_id
    };
    (directory, state, conversation)
}

fn json(value: &Prepared) -> serde_json::Value {
    let InvokeResponseBody::Json(raw) = value.response().body().unwrap() else {
        panic!("JSON response required")
    };
    serde_json::from_str(&raw).unwrap()
}

async fn read(state: &Arc<Application>, conversation: &str) -> Arc<Prepared> {
    read_conversation_since(state.clone(), conversation.into(), None, -1, false)
        .await
        .unwrap()
        .unwrap()
}

#[tokio::test]
async fn warm_reads_reuse_the_exact_response_and_preserve_watcher_and_page_contracts() {
    let (_directory, state, conversation) = fixture();
    let first = read(&state, &conversation).await;
    let changes = state.lock().unwrap().connection.total_changes();
    state
        .lock()
        .unwrap()
        .connection
        .authorizer(Some(|_: rusqlite::hooks::AuthContext<'_>| {
            rusqlite::hooks::Authorization::Deny
        }))
        .unwrap();
    let second = read(&state, &conversation).await;
    state
        .lock()
        .unwrap()
        .connection
        .authorizer(None::<fn(rusqlite::hooks::AuthContext<'_>) -> rusqlite::hooks::Authorization>)
        .unwrap();
    assert!(
        Arc::ptr_eq(&first, &second),
        "warm reads must not rebuild or re-encode"
    );
    let expected = serde_json::to_value(
        state
            .lock()
            .unwrap()
            .conversation_snapshot(&conversation, None)
            .unwrap(),
    )
    .unwrap();
    assert_eq!(json(&first), expected);
    assert!(
        read_conversation_since(
            state.clone(),
            conversation.clone(),
            None,
            first.revision,
            false
        )
        .await
        .unwrap()
        .is_none()
    );
    let timeout = read_conversation_since(
        state.clone(),
        conversation.clone(),
        None,
        first.revision,
        true,
    )
    .await
    .unwrap()
    .unwrap();
    assert!(Arc::ptr_eq(&first, &timeout));
    let page = read_conversation_since(state.clone(), conversation.clone(), Some(1), -1, false)
        .await
        .unwrap()
        .unwrap();
    assert!(!Arc::ptr_eq(&first, &page));
    assert!(Arc::ptr_eq(&first, &read(&state, &conversation).await));
    assert_eq!(state.lock().unwrap().connection.total_changes(), changes);
}

#[tokio::test]
async fn same_revision_writes_and_rollbacks_invalidate_and_errors_never_use_stale_content() {
    let (_directory, state, conversation) = fixture();
    let first = read(&state, &conversation).await;
    state
        .lock()
        .unwrap()
        .connection
        .execute("UPDATE ai_config SET route='hosted'", [])
        .unwrap();
    let changed = read(&state, &conversation).await;
    assert!(!Arc::ptr_eq(&first, &changed));
    assert_eq!(changed.revision, first.revision);
    assert_ne!(
        json(&first)["contentRevision"],
        json(&changed)["contentRevision"]
    );
    assert_eq!(json(&changed)["connection"]["route"], "hosted");
    {
        let mut store = state.lock().unwrap();
        let tx = store.connection.transaction().unwrap();
        tx.execute("UPDATE ai_config SET route='custom'", [])
            .unwrap();
        tx.rollback().unwrap();
    }
    let rolled_back = read(&state, &conversation).await;
    assert!(!Arc::ptr_eq(&changed, &rolled_back));
    let mut expected = json(&changed);
    assert_ne!(
        expected["contentRevision"],
        json(&rolled_back)["contentRevision"]
    );
    expected["contentRevision"] = json(&rolled_back)["contentRevision"].clone();
    assert_eq!(expected, json(&rolled_back));
    state
        .lock()
        .unwrap()
        .connection
        .execute(
            r#"UPDATE conversation_settings SET settings='{"bad":true}' WHERE conversation_id=?1"#,
            [&conversation],
        )
        .unwrap();
    assert!(
        read_conversation_since(state.clone(), conversation, None, -1, true)
            .await
            .is_err()
    );
}

#[tokio::test]
async fn obsolete_hydration_cannot_replace_current_data_and_fatal_state_rejects_warm_reads() {
    let (_directory, state, conversation) = fixture();
    let old = read(&state, &conversation).await;
    let version = Version::of(&state.lock().unwrap());
    state
        .lock()
        .unwrap()
        .connection
        .execute("UPDATE metadata SET revision=revision+1", [])
        .unwrap();
    let current = read(&state, &conversation).await;
    remember(&state, &version, conversation.clone(), None, old).unwrap();
    assert!(Arc::ptr_eq(&current, &read(&state, &conversation).await));
    state.clear_graph_reports();
    let cleared = read(&state, &conversation).await;
    assert!(!Arc::ptr_eq(&current, &cleared));
    state.stop(internal());
    assert!(
        read_conversation_since(state.clone(), conversation, None, -1, true)
            .await
            .is_err()
    );
}

#[tokio::test]
async fn committed_deletion_does_not_return_a_cached_conversation() {
    let (_directory, state, conversation) = fixture();
    let _ = read(&state, &conversation).await;
    {
        let mut store = state.lock().unwrap();
        let revision = store
            .snapshot()
            .unwrap()
            .conversations
            .iter()
            .find(|c| c.id == conversation)
            .unwrap()
            .revision;
        apply(
            &mut store,
            Action::DeleteConversation {
                conversation_id: conversation.clone(),
                expected_revision: revision,
            },
        );
    }
    assert!(
        read_conversation_since(state, conversation, None, -1, true)
            .await
            .is_err()
    );
}

#[tokio::test]
async fn reopening_preserves_content_and_advances_revision_without_replaying_old_receipts() {
    let (_directory, state, conversation) = fixture();
    let first = read(&state, &conversation).await;
    let command = Command {
        session_id: state.lock().unwrap().session_id.clone(),
        action_id: uuid::Uuid::new_v4().to_string(),
        action: Action::OpenConversation {
            conversation_id: conversation.clone(),
        },
    };
    let receipt = state.execute_command(command.clone()).unwrap();
    let version = Version::of(&state.lock().unwrap());
    let retained = state
        .conversation_reads
        .lock()
        .unwrap()
        .get(&version, &conversation, None)
        .expect("navigation must retain hydrated data");
    assert_eq!(retained.revision, receipt.revision);
    assert!(retained.revision > first.revision);
    let actual = read(&state, &conversation).await;
    assert!(Arc::ptr_eq(&retained, &actual));
    let mut expected = serde_json::to_value(
        state
            .lock()
            .unwrap()
            .conversation_snapshot(&conversation, None)
            .unwrap(),
    )
    .unwrap();
    // Cold reads conservatively receive a new identity after navigation.
    assert_ne!(expected["contentRevision"], json(&first)["contentRevision"]);
    expected["contentRevision"] = json(&first)["contentRevision"].clone();
    assert_eq!(json(&actual), expected);
    state
        .execute_command(Command {
            action_id: uuid::Uuid::new_v4().to_string(),
            ..command.clone()
        })
        .unwrap();
    let newer = read(&state, &conversation).await;
    state.execute_command(command).unwrap();
    assert!(Arc::ptr_eq(&newer, &read(&state, &conversation).await));
}
