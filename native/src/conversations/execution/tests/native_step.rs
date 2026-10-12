use super::{
    native_coach::{ask, tick},
    *,
};

fn control(store: &mut Store, turn: &str, control: TurnControl) {
    apply(
        store,
        Action::ControlTurn {
            turn_id: turn.into(),
            control,
        },
    );
}

#[tokio::test]
async fn native_step_runs_context_then_reply_in_two_commands_and_keeps_the_run_paused() {
    let (_dir, mut store, conversation) = setup();
    coach_provider(&store, &["Stepped reply."]);
    let turn = store.execute(ask(&store, &conversation)).unwrap().entity_id;
    control(&mut store, &turn, TurnControl::Pause);
    assert!(!tick(&mut store).await);
    let command = Command {
        session_id: store.session_id.clone(),
        action_id: id(),
        action: Action::ControlTurn {
            turn_id: turn.clone(),
            control: TurnControl::Step,
        },
    };
    store.execute(command.clone()).unwrap();
    store.execute(command).unwrap(); // receipt replay must not add a second step
    assert!(tick(&mut store).await); // local invocation
    assert!(!tick(&mut store).await); // adopt
    for _ in 0..3 {
        assert!(!tick(&mut store).await);
    }
    let snapshot = store.conversation_snapshot(&conversation, None).unwrap();
    assert_eq!(snapshot.coach_messages.len(), 1);
    let graph = snapshot.turns[0].native_graph.as_ref().unwrap();
    assert!(graph.paused && graph.step_available);
    assert!(graph.stepping.is_none());
    assert_eq!(
        graph.nodes[coach_graph::REPLY],
        crate::ai::graph::Disposition::Paused
    );
    control(&mut store, &turn, TurnControl::Step);
    assert!(
        store
            .graph_runtime
            .next(
                &mut store.connection,
                false,
                &store.config,
                &store.session_id
            )
            .unwrap()
            .is_none()
    );
    assert!(tick(&mut store).await);
    assert!(!tick(&mut store).await);
    let snapshot = store.conversation_snapshot(&conversation, None).unwrap();
    assert_eq!(snapshot.coach_messages.len(), 2);
    assert_eq!(snapshot.coach_messages[1].text, "Stepped reply.");
    assert_eq!(snapshot.turns[0].state, "succeeded");
    assert!(snapshot.turns[0].paused);
    assert!(
        snapshot.turns[0]
            .native_graph
            .as_ref()
            .unwrap()
            .stepping
            .is_none()
    );
}

#[tokio::test]
async fn native_step_is_revoked_by_explicit_pause_and_restart() {
    let (dir, mut store, conversation) = setup();
    let turn = store.execute(ask(&store, &conversation)).unwrap().entity_id;
    control(&mut store, &turn, TurnControl::Pause);
    control(&mut store, &turn, TurnControl::Step);
    control(&mut store, &turn, TurnControl::Pause);
    assert!(!tick(&mut store).await);
    control(&mut store, &turn, TurnControl::Step);
    drop(store);
    let mut store = Store::open(&dir.path().join("test.sqlite3")).unwrap();
    assert!(!tick(&mut store).await);
    let snapshot = store.conversation_snapshot(&conversation, None).unwrap();
    assert!(
        snapshot.turns[0]
            .native_graph
            .as_ref()
            .unwrap()
            .stepping
            .is_none()
    );
    control(&mut store, &turn, TurnControl::Step);
    assert!(tick(&mut store).await);
}

#[tokio::test]
async fn native_step_cannot_bypass_global_pause() {
    let (_dir, mut store, conversation) = setup();
    let turn = store.execute(ask(&store, &conversation)).unwrap().entity_id;
    control(&mut store, &turn, TurnControl::Pause);
    apply(&mut store, Action::SetPaused { paused: true });
    let command = Command {
        session_id: store.session_id.clone(),
        action_id: id(),
        action: Action::ControlTurn {
            turn_id: turn.clone(),
            control: TurnControl::Step,
        },
    };
    assert!(store.execute(command).is_err());
    assert!(!tick(&mut store).await);
    apply(&mut store, Action::SetPaused { paused: false });
    assert!(!tick(&mut store).await);
}

#[tokio::test]
async fn native_step_permission_is_rechecked_after_provider_awaits() {
    use crate::ai::graph::EvidenceLimits;
    use std::sync::Arc;
    let (_dir, mut store, conversation) = setup();
    let (sender, mut receiver) = tokio::sync::mpsc::unbounded_channel();
    let release = Arc::new(tokio::sync::Notify::new());
    let waiting = release.clone();
    store
        .graph_runtime
        .bind_provider(Arc::new(move |context, request| {
            let sender = sender.clone();
            let waiting = waiting.clone();
            Box::pin(async move {
                sender.send((context.identity().clone(), request)).unwrap();
                waiting.notified().await;
                Ok("Completed step.".into())
            })
        }));
    let turn = store.execute(ask(&store, &conversation)).unwrap().entity_id;
    control(&mut store, &turn, TurnControl::Pause);
    control(&mut store, &turn, TurnControl::Step);
    assert!(tick(&mut store).await);
    assert!(!tick(&mut store).await);
    control(&mut store, &turn, TurnControl::Step);
    let claim = store
        .graph_runtime
        .next(
            &mut store.connection,
            true,
            &store.config,
            &store.session_id,
        )
        .unwrap()
        .unwrap();
    let task = tokio::spawn(claim.invocation.execute(EvidenceLimits {
        observations: 16,
        bytes: 64_000,
    }));
    let (identity, _captured) = receiver.recv().await.unwrap();
    assert!(
        store
            .graph_runtime
            .authorize_text(&store.connection, &identity)
            .is_ok()
    );
    apply(&mut store, Action::SetPaused { paused: true });
    assert!(
        store
            .graph_runtime
            .authorize_text(&store.connection, &identity)
            .is_err()
    );
    apply(&mut store, Action::SetPaused { paused: false });
    assert!(
        store
            .graph_runtime
            .authorize_text(&store.connection, &identity)
            .is_ok()
    );
    control(&mut store, &turn, TurnControl::Pause);
    assert!(
        store
            .graph_runtime
            .authorize_text(&store.connection, &identity)
            .is_err()
    );
    release.notify_one();
    let report = task.await.unwrap();
    store
        .graph_runtime
        .finish(&mut store.connection, &conversation, &turn, report)
        .unwrap();
    assert!(!tick(&mut store).await);
    assert_eq!(
        store
            .conversation_snapshot(&conversation, None)
            .unwrap()
            .coach_messages
            .len(),
        1
    );
}
