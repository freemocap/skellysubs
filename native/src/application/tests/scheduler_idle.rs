use super::*;
use crate::model::Action;

fn deny_sql(app: &Application, deny: bool) {
    let store = app.lock().unwrap();
    if deny {
        store
            .connection
            .authorizer(Some(|_: rusqlite::hooks::AuthContext<'_>| {
                rusqlite::hooks::Authorization::Deny
            }))
            .unwrap();
    } else {
        store
            .connection
            .authorizer(
                None::<fn(rusqlite::hooks::AuthContext<'_>) -> rusqlite::hooks::Authorization>,
            )
            .unwrap();
    }
}

#[tokio::test]
async fn idle_ticks_and_navigation_do_no_sql_but_capacity_and_writes_rescan() {
    let dir = tempfile::tempdir().unwrap();
    let app = Application::start(&dir.path().join("idle.sqlite3"), None);
    let (session, conversation) = {
        let mut store = app.lock().unwrap();
        store.prepare_chat().unwrap();
        (
            store.session_id.clone(),
            store.snapshot().unwrap().conversations[0].id.clone(),
        )
    };
    let client = provider::client().unwrap();
    schedule_pending(&app, &client).await.unwrap();
    deny_sql(&app, true);
    for _ in 0..3 {
        schedule_pending(&app, &client).await.unwrap();
    }
    deny_sql(&app, false);
    app.execute_command(Command {
        session_id: session,
        action_id: uuid::Uuid::new_v4().to_string(),
        action: Action::OpenConversation {
            conversation_id: conversation,
        },
    })
    .unwrap();
    deny_sql(&app, true);
    schedule_pending(&app, &client).await.unwrap();

    let permits: Vec<_> = (0..admission::NETWORK_CAPACITY)
        .map(|_| app.admission.try_chat().unwrap())
        .collect();
    assert!(
        schedule_pending(&app, &client).await.is_err(),
        "capacity changes must rescan"
    );
    deny_sql(&app, false);
    schedule_pending(&app, &client).await.unwrap();
    deny_sql(&app, true);
    schedule_pending(&app, &client).await.unwrap();
    drop(permits);
    assert!(
        schedule_pending(&app, &client).await.is_err(),
        "released permits must rescan"
    );
    deny_sql(&app, false);
    schedule_pending(&app, &client).await.unwrap();

    // No metadata revision bump: same-revision and rolled-back writes still
    // invalidate. A later navigation must not conceal either change.
    for rollback in [false, true] {
        {
            let mut store = app.lock().unwrap();
            let tx = store.connection.transaction().unwrap();
            tx.execute("UPDATE ai_config SET paused=paused", [])
                .unwrap();
            if rollback {
                tx.rollback().unwrap();
            } else {
                tx.commit().unwrap();
            }
        }
        deny_sql(&app, true);
        assert!(schedule_pending(&app, &client).await.is_err());
        deny_sql(&app, false);
        schedule_pending(&app, &client).await.unwrap();
    }
}

#[test]
fn a_scan_that_changes_state_is_not_idle_even_without_a_claim() {
    let before = Version {
        session: "one".into(),
        changes: 10,
    };
    let after = Version {
        changes: 11,
        ..before.clone()
    };
    let mut idle = Idle::default();
    // Advance can find a retained result after the adoption pass has already
    // visited that run. None here still requires a subsequent adoption scan.
    idle.observe(before.clone(), after.clone(), true, false);
    assert!(!idle.matches(&after, true));
    idle.observe(after.clone(), after.clone(), true, false);
    assert!(idle.matches(&after, true));
    idle.navigation_committed(
        &before,
        &Version {
            changes: 12,
            ..before.clone()
        },
    );
    assert!(
        idle.matches(&after, true),
        "unobserved writes cannot be promoted"
    );
    let restarted = Version {
        session: "two".into(),
        ..after.clone()
    };
    assert!(!idle.matches(&restarted, true));
    idle.observe(after.clone(), after.clone(), true, true);
    assert!(!idle.matches(&after, true));
}
