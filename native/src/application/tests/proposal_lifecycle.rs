//! Shared execution settlement and product adoption remain separate.
use super::*;
use crate::ai::results;
use crate::application::test_server::structured_sequence;
use std::time::Duration;

#[tokio::test]
async fn failed_shared_dispatch_rolls_back_submission_and_invents_no_usage() {
    let (_dir, app) = app("http://127.0.0.1:1/v1");
    let id = reserve(&app, input()).unwrap();
    app.lock().unwrap().connection.execute_batch("CREATE TRIGGER block_execution_dispatch BEFORE INSERT ON workspace_graph_transport_identities BEGIN SELECT RAISE(ABORT,'fixture dispatch failure'); END").unwrap();
    assert_eq!(run(&app, &id).await.err().unwrap().code, ErrorCode::Storage);
    let store = app.lock().unwrap();
    let activity = generation_receipts::activity_for(&store.connection, "drill").unwrap();
    assert!(activity.attempts[0].dispatched_at.is_none());
    assert_eq!(activity.usage.attempts, 0);
    assert_eq!(store.profile().unwrap().global.attempts, 0);
    let source = crate::ai::workspace_graph::receipt(&store.connection, &id)
        .unwrap()
        .unwrap();
    assert_eq!(source["dispatched"], false);
    assert_eq!(source["state"], "failed");
    assert!(store.drill_items("spanish").unwrap().is_empty());
}

#[tokio::test]
async fn identical_generation_actions_are_fresh_and_accounted_once_each() {
    let (url, worker) = structured_sequence(2, |_| {
        json!({"candidates":[candidate("Fresh.")]}).to_string()
    });
    let (_dir, app) = app(&url);
    let first = reserve(&app, input()).unwrap();
    let second = reserve(&app, input()).unwrap();
    let a = run(&app, &first).await.unwrap();
    let b = run(&app, &second).await.unwrap();
    assert_ne!(a.candidates[0].candidate_id, b.candidates[0].candidate_id);
    let sent = worker.join().unwrap();
    assert_eq!(sent.len(), 2);
    assert_eq!(
        sent[0]["items"][0]["request"],
        sent[1]["items"][0]["request"]
    );
    assert_ne!(
        sent[0]["items"][0]["attempt_id"],
        sent[1]["items"][0]["attempt_id"]
    );
    assert!(run(&app, &first).await.is_err());
    let store = app.lock().unwrap();
    assert!(store.drill_items("spanish").unwrap().is_empty());
    let profile = store.profile().unwrap();
    assert_eq!(profile.global.attempts, 2);
    assert_eq!(profile.global.input_tokens, 42);
    assert_eq!(profile.global.output_tokens, 8);
    assert_eq!(profile.global.unknown_usage, 0);
    assert_eq!(
        profile
            .languages
            .iter()
            .find(|l| l.id == "spanish")
            .unwrap()
            .attempts,
        2
    );
    assert!(profile.personas.iter().all(|p| p.attempts == 0));
    let activity = generation_receipts::activity_for(&store.connection, "drill").unwrap();
    assert_eq!(activity.usage.attempts, 2);
    for attempt in activity.attempts {
        let source = crate::ai::workspace_graph::receipt(&store.connection, &attempt.id)
            .unwrap()
            .unwrap();
        assert_eq!(source["state"], "succeeded");
        assert_eq!(source["response"]["providerId"], "structured-receipt");
        assert!(!source.to_string().contains("Fresh."));
        assert!(!source.to_string().contains("PRIVATE-TOPIC"));
    }
    assert_eq!(
        results::settings(&store.connection).unwrap().result_count,
        0
    );
}

#[tokio::test]
async fn dropped_command_settles_submitted_execution_without_publishing_candidates() {
    let (ready, received) = tokio::sync::oneshot::channel();
    let (release, gate) = std::sync::mpsc::channel();
    let (url, worker) = structured_server(move |_| {
        ready.send(()).unwrap();
        gate.recv_timeout(Duration::from_secs(5)).unwrap();
        json!({"candidates":[candidate("PRIVATE-LATE-PROPOSAL")]}).to_string()
    });
    let (_dir, app) = app(&url);
    let id = reserve(&app, input()).unwrap();
    let task = {
        let app = app.clone();
        let id = id.clone();
        tokio::spawn(async move { run(&app, &id).await })
    };
    received.await.unwrap();
    task.abort();
    assert!(task.await.err().unwrap().is_cancelled());
    {
        let store = app.lock().unwrap();
        assert_eq!(
            generation_receipts::activity_for(&store.connection, "drill")
                .unwrap()
                .attempts[0]
                .state,
            "unknown"
        );
        assert_eq!(
            crate::ai::workspace_graph::receipt(&store.connection, &id)
                .unwrap()
                .unwrap()["state"],
            "pending"
        );
    }
    // More than the retry authority polling interval: losing the command cannot
    // cancel the provider future or turn a known completion into unknown usage.
    tokio::time::sleep(Duration::from_millis(150)).await;
    release.send(()).unwrap();
    tokio::time::timeout(Duration::from_secs(5), async {
        loop {
            let finished = {
                let store = app.lock().unwrap();
                let source = crate::ai::workspace_graph::receipt(&store.connection, &id)
                    .unwrap()
                    .unwrap();
                source["state"] == "succeeded"
            };
            if finished {
                break;
            }
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
    })
    .await
    .unwrap();
    worker.join().unwrap();
    let store = app.lock().unwrap();
    let receipt = crate::ai::workspace_graph::receipt(&store.connection, &id)
        .unwrap()
        .unwrap();
    assert_eq!(receipt["state"], "succeeded");
    assert_eq!(receipt["response"]["inputTokens"], 21);
    assert!(!receipt.to_string().contains("PRIVATE-"));
    assert!(store.drill_preview(&id).is_err());
    assert!(store.drill_items("spanish").unwrap().is_empty());
    assert_eq!(store.profile().unwrap().global.attempts, 1);
    assert_eq!(store.profile().unwrap().global.unknown_usage, 0);
    assert_eq!(
        results::settings(&store.connection).unwrap().result_count,
        0
    );
}

#[tokio::test]
async fn dropping_an_unconsumed_delivered_result_cannot_leave_a_running_proposal() {
    use std::{future::Future, task::Poll};
    let (url, worker) =
        structured_server(|_| json!({"candidates":[candidate("Unconsumed.")]}).to_string());
    let (_dir, app) = app(&url);
    let id = reserve(&app, input()).unwrap();
    let mut command = Box::pin(run(&app, &id));
    std::future::poll_fn(|cx| {
        assert!(command.as_mut().poll(cx).is_pending());
        Poll::Ready(())
    })
    .await;
    // Let the producer send its result, without polling the command's receiver.
    tokio::time::timeout(Duration::from_secs(5), async {
        loop {
            let settled = crate::ai::workspace_graph::receipt(&app.lock().unwrap().connection, &id)
                .unwrap()
                .is_some_and(|r| r["state"] == "succeeded");
            if settled {
                break;
            }
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
    })
    .await
    .unwrap();
    worker.join().unwrap();
    drop(command);
    let store = app.lock().unwrap();
    let activity = generation_receipts::activity_for(&store.connection, "drill").unwrap();
    assert_eq!(activity.attempts[0].state, "unknown");
    assert_eq!(activity.usage.input_tokens, 21);
    assert_eq!(activity.attempts[0].input_tokens, Some(21));
    assert_eq!(
        activity.attempts[0].provider_id.as_deref(),
        Some("structured-receipt")
    );
    assert!(store.drill_preview(&id).is_err());
    assert_eq!(store.profile().unwrap().global.input_tokens, 21);
}

#[tokio::test]
async fn shared_receipt_failure_refuses_publication_and_keeps_response_diagnostics() {
    let (url, worker) =
        structured_server(|_| json!({"candidates":[candidate("PRIVATE-PROPOSAL")]}).to_string());
    let (_dir, app) = app(&url);
    let id = reserve(&app, input()).unwrap();
    app.lock().unwrap().connection.execute_batch("CREATE TRIGGER block_execution_finish BEFORE UPDATE ON workspace_graph_records WHEN json_type(CAST(NEW.payload AS TEXT),'$.value.outcome')='object' BEGIN SELECT RAISE(ABORT,'fixture execution receipt failure'); END").unwrap();
    let error = run(&app, &id).await.err().unwrap();
    worker.join().unwrap();
    assert_eq!(error.code, ErrorCode::Storage);
    let diagnostic = error.diagnostics.as_ref().unwrap();
    assert_eq!(diagnostic["response"]["providerId"], "structured-receipt");
    assert!(!diagnostic.to_string().contains("PRIVATE-"));
    let store = app.lock().unwrap();
    let activity = generation_receipts::activity_for(&store.connection, "drill").unwrap();
    assert_eq!(activity.attempts[0].state, "failed");
    assert_eq!(activity.attempts[0].input_tokens, Some(21));
    assert!(store.drill_preview(&id).is_err());
    assert!(store.drill_items("spanish").unwrap().is_empty());
    assert_eq!(store.profile().unwrap().global.attempts, 1);
    assert_eq!(store.profile().unwrap().global.input_tokens, 21);
}
