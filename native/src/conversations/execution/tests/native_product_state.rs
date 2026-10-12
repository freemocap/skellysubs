//! Snapshot contracts consumed by pending bubbles, helper status and autoplay.
use super::*;
use crate::ai::graph::*;
use std::sync::Arc;

#[tokio::test]
async fn native_snapshot_exposes_captured_playback_without_legacy_operations() {
    for aloud in [false, true] {
        let (_dir, mut store, conversation) = setup();
        store.connection.execute("UPDATE conversation_settings SET settings=json_set(settings,'$.readAloud',json(?2)) WHERE conversation_id=?1", rusqlite::params![conversation, aloud.to_string()]).unwrap();
        let turn = store
            .execute(send(&store, &conversation))
            .unwrap()
            .entity_id;
        store.graph_runtime.partner.bind(
            Arc::new(|invocation| {
                Box::pin(async move {
                    if invocation.identity().operation == prose::operation_contract() {
                        Ok(reply("Hola."))
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
        let pending = store.conversation_snapshot(&conversation, None).unwrap();
        assert_eq!(pending.turns[0].state, "pending");
        assert_eq!(pending.turns[0].native_execution_available, Some(true));
        assert!(pending.turns[0].speech.is_none());
        for _ in 0..40 {
            native_coach::tick(&mut store).await;
            let snapshot = store.conversation_snapshot(&conversation, None).unwrap();
            if snapshot.messages.iter().any(|m| m.role == "assistant") {
                break;
            }
        }
        let snapshot = store.conversation_snapshot(&conversation, None).unwrap();
        let assistant = snapshot
            .messages
            .iter()
            .find(|m| m.role == "assistant")
            .unwrap();
        let speech = snapshot
            .turns
            .iter()
            .find(|t| t.id == turn)
            .unwrap()
            .speech
            .as_ref();
        assert_eq!(speech.is_some(), aloud);
        if let Some(speech) = speech {
            assert_eq!(speech.source_message_id, assistant.id);
            assert_eq!(
                Some(speech.id.clone()),
                store
                    .graph_runtime
                    .requested_speech(&store.connection, &assistant.id)
                    .unwrap()
            );
        }
        let legacy: i64 = store
            .connection
            .query_row(
                "SELECT COUNT(*) FROM operations WHERE turn_id=?1",
                [&turn],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(legacy, 0);
        // No provider capacity is a queued state, not a pause or a failure.
        for _ in 0..20 {
            let claim = store
                .graph_runtime
                .next(
                    &mut store.connection,
                    false,
                    &store.config,
                    &store.session_id,
                )
                .unwrap();
            if let Some(claim) = claim {
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
            }
        }
        let queued = store.conversation_snapshot(&conversation, None).unwrap();
        let assistant = queued
            .messages
            .iter()
            .find(|m| m.role == "assistant")
            .unwrap();
        assert_eq!(assistant.translation_state.as_deref(), Some("ready"));
    }
}

#[tokio::test]
async fn reply_publishes_while_independent_assessment_is_still_running() {
    let (_dir, mut store, conversation) = setup();
    let turn = store
        .execute(send(&store, &conversation))
        .unwrap()
        .entity_id;
    store.graph_runtime.partner.bind(
        Arc::new(|invocation| {
            Box::pin(async move {
                if invocation.identity().operation == prose::operation_contract() {
                    Ok(reply("Hola."))
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
    let mut assessment = None;
    for _ in 0..40 {
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
            if claim.resource == Resource::Provider && assessment.is_none() {
                assessment = Some(claim);
                continue; // The provider has not returned; independent work must progress.
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
                .finish(&mut store.connection, &conversation, &turn, report)
                .unwrap();
        }
        if store
            .conversation_snapshot(&conversation, None)
            .unwrap()
            .messages
            .iter()
            .any(|m| m.role == "assistant")
        {
            assert!(assessment.is_some(), "assessment must still be in flight");
            let snapshot = store.conversation_snapshot(&conversation, None).unwrap();
            let learner = snapshot
                .messages
                .iter()
                .find(|message| message.role == "user")
                .unwrap();
            assert!(matches!(
                learner.assessment_state.as_deref(),
                Some("running" | "waiting_dependencies")
            ));
            assert!(learner.assessment_error.is_none());

            return;
        }
    }
    panic!("reply was blocked behind the unfinished assessment");
}

#[test]
fn local_work_has_no_network_slot_ceiling_even_without_provider_capacity() {
    let (_dir, mut store, conversation) = setup();
    let count = crate::ai::policy::admission::NETWORK_CAPACITY + 1;
    let mut registry = Registry::default();
    context::register(&mut registry).unwrap();
    prose::register(
        &mut registry,
        Arc::new(|_, _| Box::pin(async { Ok("Reply".into()) })),
    )
    .unwrap();
    let operation = Contract::new("fixture.local-work", 1);
    registry
        .register(
            Operation {
                contract: operation.clone(),
                implementation: "fixture/local/1".into(),
                inputs: Default::default(),
                outputs: Default::default(),
                resource: Resource::Local,
                reuse: Reuse::Fresh,
            },
            Arc::new(|_, _| Box::pin(async { Ok(Default::default()) })),
        )
        .unwrap();
    let mut definition = store.graph_runtime.graph.artifact().definition.clone();
    definition.contract.version += 1;
    for index in 0..count {
        definition.nodes.insert(
            format!("local-{index}"),
            Node {
                operation: operation.clone(),
                inputs: Default::default(),
                after: vec![],
                guard: None,
                activation: Activation::Automatic,
            },
        );
    }
    let graph = Arc::new(registry.compile(definition).unwrap());
    store.graph_runtime.catalog = store
        .graph_runtime
        .register_artifact(graph.clone())
        .unwrap();
    store.graph_runtime.graph = graph;
    let turn = store
        .execute(native_coach::ask(&store, &conversation))
        .unwrap()
        .entity_id;
    // Hold two invocations unfinished: local work does not serialize behind the
    // first. All remaining local nodes must already be admitted, including those
    // beyond the network pool size. Avoid exhausting unrelated settlement-byte
    // reservations by needlessly claiming every producer without completing it.
    let mut running = Vec::new();
    for _ in 0..2 {
        let claim = store
            .graph_runtime
            .next(
                &mut store.connection,
                false,
                &store.config,
                &store.session_id,
            )
            .unwrap()
            .expect("ready local work must not wait for network slots or other local work");
        assert_eq!(claim.resource, Resource::Local);
        running.push(claim);
    }
    assert_eq!(running.len(), 2);
    let view = store
        .graph_runtime
        .inspection(&store.connection, &conversation, &turn)
        .unwrap()
        .unwrap();
    assert_eq!(
        view.nodes
            .values()
            .filter(|state| matches!(state, Disposition::Prepared | Disposition::Running))
            .count(),
        count + 1
    ); // fixture locals plus context
}
