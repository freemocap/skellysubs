use super::*;
use crate::model::Action;

fn fixture() -> (tempfile::TempDir, Arc<Application>, String) {
    let dir = tempfile::tempdir().unwrap();
    let app = Application::start(&dir.path().join("learning.sqlite3"), None);
    let conversation = {
        let mut store = app.lock().unwrap();
        store.prepare_chat().unwrap();
        store.snapshot().unwrap().conversations[0].id.clone()
    };
    (dir, app, conversation)
}

fn deny_sql(app: &Application, deny: bool) {
    let store = app.lock().unwrap();
    store
        .connection
        .authorizer(if deny {
            Some(
                (|_: rusqlite::hooks::AuthContext<'_>| rusqlite::hooks::Authorization::Deny)
                    as fn(rusqlite::hooks::AuthContext<'_>) -> rusqlite::hooks::Authorization,
            )
        } else {
            None
        })
        .unwrap();
}

fn displays(app: &Application, conversation: &str) -> serde_json::Value {
    serde_json::json!({
        "evidence": app.read_skill_evidence("spanish").unwrap(),
        "totals": app.read_language_totals().unwrap(),
        "effort": app.read_effort_progress("spanish", None).unwrap(),
        "conversation": app.read_effort_progress("spanish", Some(conversation)).unwrap(),
    })
}

#[test]
fn learning_displays_survive_navigation_without_sql_and_match_fresh_projections() {
    let (_dir, app, conversation) = fixture();
    let first = displays(&app, &conversation);
    let session = app.lock().unwrap().session_id.clone();
    app.execute_command(Command {
        session_id: session,
        action_id: uuid::Uuid::new_v4().to_string(),
        action: Action::OpenConversation {
            conversation_id: conversation.clone(),
        },
    })
    .unwrap();
    deny_sql(&app, true);
    assert_eq!(displays(&app, &conversation), first);
    assert!(
        app.read_skill_evidence("french").is_err(),
        "selection must be part of the key"
    );
    assert!(
        app.read_effort_progress("spanish", Some("another"))
            .is_err()
    );
    deny_sql(&app, false);
    app.clear_graph_reports();
    assert_eq!(displays(&app, &conversation), first);
    let store = app.lock().unwrap();
    assert_eq!(
        first["evidence"],
        progression::snapshot(&store, "spanish").unwrap()
    );
    assert_eq!(
        first["totals"],
        serde_json::to_value(progression::language_totals(&store).unwrap()).unwrap()
    );
}

#[test]
fn awards_claims_rollback_and_invalid_data_never_reuse_stale_learning_displays() {
    let (_dir, app, conversation) = fixture();
    let first = displays(&app, &conversation);
    let revision = app.lock().unwrap().snapshot().unwrap().revision;
    app.lock().unwrap().connection.execute(
        "INSERT INTO effort_awards(id,dimension,source_id,language_id,variety_id,conversation_id,policy) VALUES('award','explorations','source','spanish','standard',?1,'fixture')",
        [&conversation],
    ).unwrap();
    let awarded = displays(&app, &conversation);
    assert_ne!(awarded["effort"], first["effort"]);
    assert_eq!(awarded["effort"]["explorations"], 1);
    assert_eq!(awarded["conversation"]["explorations"], 1);
    assert_eq!(app.lock().unwrap().snapshot().unwrap().revision, revision);
    app.lock()
        .unwrap()
        .connection
        .execute("UPDATE effort_awards SET claimed=1", [])
        .unwrap();
    assert_eq!(
        displays(&app, &conversation)["effort"]["recent"][0]["claimed"],
        true
    );
    {
        let mut store = app.lock().unwrap();
        let tx = store.connection.transaction().unwrap();
        tx.execute("UPDATE effort_awards SET claimed=0", [])
            .unwrap();
        tx.rollback().unwrap();
    }
    deny_sql(&app, true);
    assert!(app.read_effort_progress("spanish", None).is_err());
    deny_sql(&app, false);
    assert_eq!(
        displays(&app, &conversation)["effort"]["recent"][0]["claimed"],
        true
    );
    // A failed validation after a write is an error, never a stale cache hit.
    app.lock()
        .unwrap()
        .connection
        .execute("UPDATE learner SET preferences='{}'", [])
        .unwrap();
    assert!(app.read_skill_evidence("spanish").is_err());
}
