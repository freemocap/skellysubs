use super::*;
use crate::ai::graph as g;
use std::sync::Arc;

pub(super) fn ask(store: &Store, conversation: &str) -> Command {
    Command {
        session_id: store.session_id.clone(),
        action_id: id(),
        action: Action::AskCoach {
            conversation_id: conversation.into(),
            text: "Explain this.".into(),
            expected_revision: store
                .snapshot()
                .unwrap()
                .conversations
                .iter()
                .find(|c| c.id == conversation)
                .unwrap()
                .revision,
        },
    }
}

pub(super) async fn tick(store: &mut Store) -> bool {
    if let Some(claim) = store
        .graph_runtime
        .next(
            &mut store.connection,
            true,
            &store.config,
            &store.session_id,
        )
        .unwrap()
    {
        let report = claim
            .invocation
            .execute(g::EvidenceLimits {
                observations: 16,
                bytes: 64_000,
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
        true
    } else {
        false
    }
}

#[tokio::test]
async fn native_coach_routes_multiple_catalogs_by_owner_and_retains_unavailable_graphs() {
    let (dir, mut store, conversation) = setup();
    coach_provider(&store, &["First catalog reply."]);
    let first = store.execute(ask(&store, &conversation)).unwrap().entity_id;
    for _ in 0..4 {
        tick(&mut store).await;
    }
    let original = store
        .graph_runtime
        .inspection(&store.connection, &conversation, &first)
        .unwrap()
        .unwrap();
    let mut registry = g::Registry::default();
    context::register(&mut registry).unwrap();
    prose::register(
        &mut registry,
        Arc::new(|_, _| Box::pin(async { Ok("Second catalog reply.".into()) })),
    )
    .unwrap();
    let mut definition = store.graph_runtime.graph.artifact().definition.clone();
    definition.contract.version += 1;
    let second_graph = Arc::new(registry.compile(definition).unwrap());
    store.graph_runtime.catalog = store
        .graph_runtime
        .register_artifact(second_graph.clone())
        .unwrap();
    store.graph_runtime.graph = second_graph.clone();
    let second = store.execute(ask(&store, &conversation)).unwrap().entity_id;
    for _ in 0..4 {
        tick(&mut store).await;
    }
    let first_view = store
        .graph_runtime
        .inspection(&store.connection, &conversation, &first)
        .unwrap()
        .unwrap();
    let second_view = store
        .graph_runtime
        .inspection(&store.connection, &conversation, &second)
        .unwrap()
        .unwrap();
    assert_eq!(first_view.engine, original.engine);
    assert_eq!(first_view.artifact_id, original.artifact_id);
    assert_ne!(first_view.engine, second_view.engine);
    assert_ne!(first_view.artifact_id, second_view.artifact_id);
    drop(store);
    let store = Store::open(&dir.path().join("test.sqlite3")).unwrap();
    let snapshot = store.conversation_snapshot(&conversation, None).unwrap();
    let retained = snapshot.turns.iter().find(|t| t.id == second).unwrap();
    assert_eq!(retained.native_execution_available, Some(false));
    assert_eq!(
        retained.native_graph.as_ref().unwrap().artifact_id,
        second_view.artifact_id
    );
    assert_eq!(
        retained.native_graph.as_ref().unwrap().nodes,
        second_view.nodes
    );
    assert_eq!(snapshot.coach_messages.len(), 4);
    let history = store
        .graph_runtime
        .history(&store.connection, &conversation, &second, None, 32)
        .unwrap();
    assert!(!history.frames.is_empty());
    assert!(
        history
            .frames
            .iter()
            .all(|frame| frame.artifact_id == second_view.artifact_id
                && frame.engine == second_view.engine)
    );
    assert_eq!(history.frames.last().unwrap().nodes, second_view.nodes);
    for cursor in ["01", "0", "18446744073709551616"] {
        assert!(
            store
                .graph_runtime
                .history(&store.connection, &conversation, &second, Some(cursor), 32)
                .is_err()
        );
    }
    assert!(
        store
            .graph_runtime
            .history(&store.connection, "other-conversation", &second, None, 32)
            .is_err()
    );
    assert!(
        store
            .graph_runtime
            .inspection(&store.connection, "other-conversation", &first)
            .is_err()
    );
}

#[test]
fn native_admission_failure_rolls_back_the_entire_command_and_receipt() {
    let (_dir, mut store, conversation) = setup();
    let command = ask(&store, &conversation);
    store.connection.execute_batch("CREATE TEMP TRIGGER reject_graph BEFORE INSERT ON graph_engines BEGIN SELECT RAISE(ABORT,'fixture rejection'); END;").unwrap();
    assert!(store.execute(command.clone()).is_err());
    for table in [
        "turns",
        "messages",
        "graph_engines",
        "turn_execution_owners",
        "conversation_graph_effects",
    ] {
        assert_eq!(
            store
                .connection
                .query_row(&format!("SELECT count(*) FROM {table}"), [], |r| r
                    .get::<_, i64>(0))
                .unwrap(),
            0,
            "{table}"
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
        .execute_batch("DROP TRIGGER reject_graph")
        .unwrap();
    store.execute(command).unwrap();
}

#[tokio::test]
async fn native_stream_prefix_survives_cancellation_without_publication_or_double_usage() {
    let (_dir, mut store, conversation) = setup();
    let (sender, mut receiver) = tokio::sync::mpsc::unbounded_channel();
    let released = Arc::new(tokio::sync::Notify::new());
    let release = released.clone();
    store
        .graph_runtime
        .bind_provider(Arc::new(move |context, _| {
            let sender = sender.clone();
            let release = release.clone();
            Box::pin(async move {
                context.provisional_text("Private partial reply")?;
                context.observe(g::ResponseEvidence {
                    usage: Some(g::UsageEvidence {
                        input_tokens: Some(21),
                        output_tokens: Some(8),
                        total_tokens: None,
                        provenance: "fixture".into(),
                    }),
                    ..Default::default()
                })?;
                sender.send(context.snapshot()?).unwrap();
                release.notified().await;
                Ok("Late completed reply".into())
            })
        }));
    let turn = store.execute(ask(&store, &conversation)).unwrap().entity_id;
    let definition = serde_json::to_value(
        store
            .conversation_snapshot(&conversation, None)
            .unwrap()
            .turns[0]
            .native_graph
            .as_ref()
            .unwrap()
            .artifact
            .clone(),
    )
    .unwrap();
    tick(&mut store).await; // finish context; the next call adopts and claims reply
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
    let running = tokio::spawn(claim.invocation.execute_with_provisional(
        g::EvidenceLimits {
            observations: 16,
            bytes: 64_000,
        },
        g::ProvisionalLimits { bytes: 64_000 },
    ));
    store
        .graph_runtime
        .observe(&mut store.connection, receiver.recv().await.unwrap())
        .unwrap();
    let view = store.conversation_snapshot(&conversation, None).unwrap();
    assert_eq!(
        view.turns[0].native_preview.as_ref().unwrap().capture.text,
        "Private partial reply"
    );
    assert!(view.turns[0].native_preview.as_ref().unwrap().live);
    let response = view.turns[0].native_response.as_ref().unwrap();
    assert_eq!(response["complete"], false);
    assert_eq!(response["observations"][0]["usage"]["input_tokens"], 21);
    assert_eq!(
        serde_json::to_value(&view.turns[0].native_graph.as_ref().unwrap().artifact).unwrap(),
        definition
    );
    apply(
        &mut store,
        Action::ControlTurn {
            turn_id: turn.clone(),
            control: TurnControl::Cancel,
        },
    );
    released.notify_one();
    store
        .graph_runtime
        .finish(
            &mut store.connection,
            &conversation,
            &turn,
            running.await.unwrap(),
        )
        .unwrap();
    assert!(!tick(&mut store).await);
    let view = store.conversation_snapshot(&conversation, None).unwrap();
    assert_eq!(view.coach_messages.len(), 1);
    assert!(!view.turns[0].native_preview.as_ref().unwrap().live);
    let response = view.turns[0].native_response.as_ref().unwrap();
    assert_eq!(response["complete"], true);
    assert_eq!(response["observations"][0]["usage"]["input_tokens"], 21);
    let usage = store.profile().unwrap().global;
    assert_eq!(
        (
            usage.attempts,
            usage.input_tokens,
            usage.output_tokens,
            usage.unknown_usage
        ),
        (1, 21, 8, 0)
    );
}

#[tokio::test]
async fn coach_command_replay_executes_and_publishes_once_without_legacy_operations() {
    let (dir, mut store, conversation) = setup();
    store.graph_runtime.bind_provider(Arc::new(|_, request| {
        Box::pin(async move {
            assert_eq!(
                request.context.messages.last().unwrap().content,
                "Explain this."
            );
            Ok("A useful explanation.".into())
        })
    }));
    let command = ask(&store, &conversation);
    let turn = store.execute(command.clone()).unwrap().entity_id;
    assert_eq!(store.execute(command).unwrap().entity_id, turn);
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
            .query_row("SELECT state FROM turns WHERE id=?1", [&turn], |r| r
                .get::<_, String>(0))
            .unwrap(),
        "pending"
    );
    for _ in 0..8 {
        tick(&mut store).await;
    }
    let snapshot = store.conversation_snapshot(&conversation, None).unwrap();
    assert_eq!(snapshot.coach_messages.len(), 2);
    assert_eq!(snapshot.coach_messages[1].text, "A useful explanation.");
    let award: String = store
        .connection
        .query_row(
            "SELECT source_id FROM effort_awards WHERE source_id LIKE 'graph-effect:%'",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert!(
        snapshot.turns[0]
            .award_sources
            .as_ref()
            .unwrap()
            .contains(&award)
    );

    assert_eq!(snapshot.turns[0].state, "succeeded");
    assert_eq!(
        store
            .connection
            .query_row(
                "SELECT count(*) FROM conversation_graph_publications",
                [],
                |r| r.get::<_, i64>(0)
            )
            .unwrap(),
        1
    );
    assert_eq!(
        store
            .connection
            .query_row(
                "SELECT count(*) FROM effort_awards WHERE source_id LIKE 'graph-effect:%'",
                [],
                |r| r.get::<_, i64>(0)
            )
            .unwrap(),
        1
    );
    drop(store);
    let mut store = Store::open(&dir.path().join("test.sqlite3")).unwrap();
    assert!(!tick(&mut store).await);
    assert_eq!(
        store
            .conversation_snapshot(&conversation, None)
            .unwrap()
            .coach_messages
            .len(),
        2
    );
}

#[tokio::test]
async fn interrupted_native_coach_recovers_unknown_and_requires_explicit_retry() {
    let (dir, mut store, conversation) = setup();
    let turn = store.execute(ask(&store, &conversation)).unwrap().entity_id;
    assert!(tick(&mut store).await); // local context
    // The next claim adopts context and dispatches the provider in one pass.
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
    assert_eq!(claim.resource, g::Resource::Provider);
    drop(claim);
    drop(store);
    let mut store = Store::open(&dir.path().join("test.sqlite3")).unwrap();
    assert_eq!(
        store
            .conversation_snapshot(&conversation, None)
            .unwrap()
            .turns[0]
            .state,
        "unknown"
    );
    assert!(!tick(&mut store).await);
    store.graph_runtime.bind_provider(Arc::new(|_, _| {
        Box::pin(async { Ok("Retried explicitly.".into()) })
    }));
    apply(
        &mut store,
        Action::ControlTurn {
            turn_id: turn,
            control: TurnControl::Retry,
        },
    );
    for _ in 0..5 {
        tick(&mut store).await;
    }
    assert_eq!(
        store
            .conversation_snapshot(&conversation, None)
            .unwrap()
            .coach_messages[1]
            .text,
        "Retried explicitly."
    );
}

#[tokio::test]
async fn selected_native_attempt_inspection_is_read_only_and_revision_bound() {
    let (_dir, mut store, conversation) = setup();
    coach_provider(&store, &["Retained inspection reply."]);
    let run = store.execute(ask(&store, &conversation)).unwrap().entity_id;
    for _ in 0..4 {
        tick(&mut store).await;
    }
    let snapshot = store
        .graph_runtime
        .inspection(&store.connection, &conversation, &run)
        .unwrap()
        .unwrap();
    let attempt = snapshot.attempts["reply"].last().unwrap();
    crate::ai::inspection::verify_workspace_reads(&store);
    let changes = store.connection.total_changes();
    let detail = crate::ai::inspection::attempt(
        &store.connection,
        &snapshot.engine,
        &run,
        &snapshot.revision,
        "reply",
        &attempt.id,
    )
    .unwrap();
    assert_eq!(detail.execution, attempt.execution);
    assert!(
        detail
            .inputs
            .as_object()
            .is_some_and(|inputs| !inputs.is_empty())
    );
    assert!(
        detail
            .outcome
            .as_ref()
            .unwrap()
            .to_string()
            .contains("Retained inspection reply.")
    );
    assert!(
        !serde_json::to_string(&snapshot)
            .unwrap()
            .contains("Retained inspection reply.")
    );
    assert_eq!(detail.owner, conversation);
    assert_eq!(detail.artifact, snapshot.artifact_id);
    assert!(
        crate::ai::inspection::attempt(
            &store.connection,
            &snapshot.engine,
            &run,
            "0",
            "reply",
            &attempt.id
        )
        .is_err()
    );
    assert!(
        crate::ai::inspection::attempt(
            &store.connection,
            &snapshot.engine,
            &run,
            &snapshot.revision,
            "context",
            &attempt.id
        )
        .is_err()
    );
    assert!(
        crate::ai::inspection::attempt(
            &store.connection,
            "other",
            &run,
            &snapshot.revision,
            "reply",
            &attempt.id
        )
        .is_err()
    );
    assert_eq!(store.connection.total_changes(), changes);
    use crate::learning::effort::bot;
    assert!(
        bot::inspect(
            &mut store.connection,
            &snapshot.engine,
            &run,
            &snapshot.revision,
            "context",
            &attempt.id
        )
        .is_err()
    );
    let earned = bot::inspect(
        &mut store.connection,
        &snapshot.engine,
        &run,
        &snapshot.revision,
        "reply",
        &attempt.id,
    )
    .unwrap()
    .unwrap();
    assert_eq!(
        earned.conversation_id.as_deref(),
        Some(conversation.as_str())
    );
    let changes = store.connection.total_changes();
    assert!(
        bot::inspect(
            &mut store.connection,
            &snapshot.engine,
            &run,
            &snapshot.revision,
            "reply",
            &attempt.id
        )
        .unwrap()
        .is_none()
    );
    assert_eq!(store.connection.total_changes(), changes);
}
