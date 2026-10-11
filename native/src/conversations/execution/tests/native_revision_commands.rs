//! Production command revisions and controls, without legacy admission fixtures.
use super::*;
use crate::ai::graph::*;
use std::sync::Arc;
fn bind(store: &Store) {
    store.graph_runtime.partner.bind(
        Arc::new(|invocation| {
            Box::pin(async move {
                if invocation.identity().operation == prose::operation_contract() {
                    Ok(reply("Respuesta nueva."))
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
}

fn assert_batched_statuses_match_individual_sources(store: &Store, conversation: &str) {
    let page = store.conversation_snapshot(conversation, None).unwrap();
    let mut individual = page.messages.clone();
    for message in &mut individual {
        store
            .graph_runtime
            .message_statuses(&store.connection, std::slice::from_mut(message))
            .unwrap();
    }
    assert_eq!(
        serde_json::to_value(&page.messages).unwrap(),
        serde_json::to_value(&individual).unwrap()
    );
    individual.reverse();
    store
        .graph_runtime
        .message_statuses(&store.connection, &mut individual)
        .unwrap();
    individual.reverse();
    assert_eq!(
        serde_json::to_value(&page.messages).unwrap(),
        serde_json::to_value(&individual).unwrap()
    );
}
async fn tick(store: &mut Store) -> bool {
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
        return false;
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
    true
}
async fn claim_reply(store: &mut Store, conversation: &str, turn: &str) -> graph_runtime::Claim {
    for _ in 0..50 {
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
        let view = store
            .graph_runtime
            .inspection(&store.connection, conversation, turn)
            .unwrap()
            .unwrap();
        if claim.run == turn
            && claim.resource == Resource::Provider
            && view.nodes["reply"] == Disposition::Running
        {
            return claim;
        }
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
    panic!("reply provider was never dispatched");
}
async fn drive(store: &mut Store) {
    for _ in 0..45 {
        tick(store).await;
    }
}
fn control(store: &mut Store, turn: &str, control: TurnControl) {
    apply(
        store,
        Action::ControlTurn {
            turn_id: turn.into(),
            control,
        },
    );
}
fn count(store: &Store, sql: &str, turn: &str) -> i64 {
    store
        .connection
        .query_row(sql, [turn], |r| r.get(0))
        .unwrap()
}

#[tokio::test]
async fn native_revision_commands_preserve_chain_and_discard_suffix_with_late_work() {
    let (dir, mut store, conversation) = setup();
    bind(&store);
    let first = store
        .execute(send(&store, &conversation))
        .unwrap()
        .entity_id;
    drive(&mut store).await;
    coach_provider(&store, &["Retained private coach answer."]);
    let coach = store
        .execute(native_coach::ask(&store, &conversation))
        .unwrap()
        .entity_id;
    drive(&mut store).await;
    let later = store
        .execute(send(&store, &conversation))
        .unwrap()
        .entity_id;
    let claim = claim_reply(&mut store, &conversation, &later).await;
    assert_eq!(claim.run, later);
    let source = store.conversation_snapshot(&conversation, None).unwrap();
    assert_eq!(
        source
            .revision_suffix_counts
            .iter()
            .find(|c| c.turn_id == first)
            .unwrap()
            .exchange_count,
        1
    );
    let obsolete_gloss = source
        .messages
        .iter()
        .find(|m| m.turn_id == first && m.role == "user")
        .unwrap()
        .gloss_operation_id
        .clone()
        .unwrap();
    let text = "Cafe\u{301} — 日本語 مرحبا";
    let command = revision_command(&store, &conversation, &first, text);
    store.connection.execute_batch("CREATE TEMP TRIGGER reject_revision BEFORE UPDATE ON graph_engines BEGIN SELECT RAISE(ABORT,'fixture rejection'); END;").unwrap();
    assert!(store.execute(command.clone()).is_err());
    assert_eq!(
        count(&store, "SELECT count(*) FROM turns WHERE id=?1", &later),
        1
    );
    store
        .connection
        .execute_batch("DROP TRIGGER reject_revision")
        .unwrap();
    let replacement = store.execute(command.clone()).unwrap().entity_id;
    assert_eq!(store.execute(command).unwrap().entity_id, replacement);
    assert_eq!(
        count(
            &store,
            "SELECT count(*) FROM turn_execution_owners WHERE turn_id=?1 AND executor='graph'",
            &replacement
        ),
        1
    );
    assert_eq!(
        count(&store, "SELECT count(*) FROM turns WHERE id=?1", &later),
        0
    );
    assert_eq!(
        count(
            &store,
            "SELECT count(*) FROM messages WHERE turn_id=?1",
            &coach
        ),
        2
    );
    let stale_request = Command {
        session_id: store.session_id.clone(),
        action_id: id(),
        action: Action::RetryGloss {
            operation_id: obsolete_gloss,
        },
    };
    assert_eq!(
        store.execute(stale_request).unwrap_err().code,
        ErrorCode::Conflict
    );
    let report = claim
        .invocation
        .execute(EvidenceLimits {
            observations: 16,
            bytes: 65536,
        })
        .await;
    store
        .graph_runtime
        .finish(&mut store.connection, &conversation, &later, report)
        .unwrap();
    drive(&mut store).await;
    assert_eq!(
        count(
            &store,
            "SELECT count(*) FROM messages WHERE turn_id=?1",
            &later
        ),
        0
    );
    assert_eq!(
        count(
            &store,
            "SELECT count(*) FROM messages WHERE turn_id=?1 AND role='assistant'",
            &replacement
        ),
        1
    );
    assert_eq!(
        count(
            &store,
            "SELECT count(*) FROM messages WHERE turn_id=?1",
            &coach
        ),
        2
    );
    let source = store
        .conversation_snapshot(&conversation, None)
        .unwrap()
        .messages
        .into_iter()
        .find(|m| m.turn_id == first && m.role == "user")
        .unwrap();
    assert_eq!(
        store
            .message_history(&conversation, &source.id)
            .unwrap()
            .versions
            .len(),
        2
    );
    assert_eq!(
        store
            .connection
            .query_row(
                "SELECT text FROM messages WHERE turn_id=?1 AND role='user'",
                [&replacement],
                |r| r.get::<_, String>(0)
            )
            .unwrap(),
        text
    );
    let regenerated = store
        .execute(revision_command(&store, &conversation, &replacement, text))
        .unwrap()
        .entity_id;
    assert_ne!(regenerated, replacement);
    drive(&mut store).await;
    assert_eq!(
        count(
            &store,
            "SELECT count(*) FROM operations WHERE turn_id IN (SELECT id FROM turns WHERE conversation_id=?1)",
            &conversation
        ),
        0
    );
    drop(store);
    let store = Store::open(&dir.path().join("test.sqlite3")).unwrap();
    assert_eq!(
        store
            .message_history(&conversation, &source.id)
            .unwrap()
            .versions
            .len(),
        3
    );
    assert_eq!(
        count(
            &store,
            "SELECT count(*) FROM messages WHERE turn_id=?1 AND role='assistant'",
            &regenerated
        ),
        1
    );
}
#[tokio::test]
async fn native_revision_commands_pause_step_resume_cancel_keep_topology() {
    let (_dir, mut store, conversation) = setup();
    bind(&store);
    let turn = store
        .execute(send(&store, &conversation))
        .unwrap()
        .entity_id;
    let before = store
        .graph_runtime
        .inspection(&store.connection, &conversation, &turn)
        .unwrap()
        .unwrap();
    control(&mut store, &turn, TurnControl::Pause);
    assert!(!tick(&mut store).await);
    control(&mut store, &turn, TurnControl::Step);
    assert!(tick(&mut store).await);
    assert!(!tick(&mut store).await);
    control(&mut store, &turn, TurnControl::Resume);
    let claim = claim_reply(&mut store, &conversation, &turn).await;
    control(&mut store, &turn, TurnControl::Cancel);
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
    assert!(!tick(&mut store).await);
    assert_eq!(
        count(
            &store,
            "SELECT count(*) FROM messages WHERE turn_id=?1 AND role='assistant'",
            &turn
        ),
        0
    );
    let after = store
        .graph_runtime
        .inspection(&store.connection, &conversation, &turn)
        .unwrap()
        .unwrap();
    assert_eq!(after.artifact_id, before.artifact_id);
    assert_eq!(
        after.nodes.keys().collect::<Vec<_>>(),
        before.nodes.keys().collect::<Vec<_>>()
    );
}
#[tokio::test]
async fn native_revision_commands_retry_gloss_uses_exact_native_operation() {
    let (_dir, mut store, conversation) = setup();
    bind(&store);
    let turn = store
        .execute(send(&store, &conversation))
        .unwrap()
        .entity_id;
    drive(&mut store).await;
    assert_batched_statuses_match_individual_sources(&store, &conversation);
    let message = store
        .conversation_snapshot(&conversation, None)
        .unwrap()
        .messages
        .into_iter()
        .find(|m| m.turn_id == turn && m.role == "user")
        .unwrap();
    let operation = message.gloss_operation_id.unwrap();
    assert_eq!(message.gloss_state.as_deref(), Some("failed"));
    let receipt = apply(
        &mut store,
        Action::RetryGloss {
            operation_id: operation.clone(),
        },
    );
    assert_eq!(receipt.entity_id, operation);
    let after = store
        .graph_runtime
        .inspection(&store.connection, &conversation, &turn)
        .unwrap()
        .unwrap();
    assert_eq!(after.nodes["learner_gloss"], Disposition::Ready);
    assert_batched_statuses_match_individual_sources(&store, &conversation);
    drive(&mut store).await;
    assert_batched_statuses_match_individual_sources(&store, &conversation);
    assert_eq!(
        count(
            &store,
            "SELECT count(*) FROM operations WHERE turn_id=?1",
            &turn
        ),
        0
    );
}

#[tokio::test]
async fn native_revision_commands_restart_requires_explicit_retry() {
    let (dir, mut store, conversation) = setup();
    bind(&store);
    let turn = store
        .execute(send(&store, &conversation))
        .unwrap()
        .entity_id;
    // Finish local preparation, then leave an actual provider execution in flight.
    let claim = claim_reply(&mut store, &conversation, &turn).await;
    drop(claim);
    drop(store);
    let mut store = Store::open(&dir.path().join("test.sqlite3")).unwrap();
    bind(&store);
    let before = store
        .graph_runtime
        .inspection(&store.connection, &conversation, &turn)
        .unwrap()
        .unwrap();
    assert_eq!(before.nodes["reply"], Disposition::Unknown);
    assert!(!tick(&mut store).await);
    control(&mut store, &turn, TurnControl::Retry);
    drive(&mut store).await;
    assert_eq!(
        count(
            &store,
            "SELECT count(*) FROM messages WHERE turn_id=?1 AND role='assistant'",
            &turn
        ),
        1
    );
    let after = store
        .graph_runtime
        .inspection(&store.connection, &conversation, &turn)
        .unwrap()
        .unwrap();
    assert_eq!(
        after.attempts["reply"].len(),
        before.attempts["reply"].len() + 1
    );
    assert_eq!(after.artifact_id, before.artifact_id);
}
