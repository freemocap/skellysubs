use super::*;
use crate::{
    ai::results,
    application::test_server::{structured_sequence, translation_server},
};
use serde_json::json;

fn input(aid: reading::ReadingAid) -> reading::ReadingInput {
    reading::ReadingInput {
        conversation_id: None,
        reference_item: None,
        text: "Hola casa".into(),
        language: "spanish".into(),
        variety: None,
        explanation: "english".into(),
        explanation_variety: None,
        aid,
    }
}
fn configure(state: &Application, base: &str) {
    state.lock().unwrap().connection.execute("UPDATE ai_config SET route='custom',custom_config=json_set(custom_config,'$.baseUrl',?1,'$.bearerAuth',json('false'))", [base]).unwrap();
}
fn begin(state: &Application, aid: reading::ReadingAid, fresh: bool) -> String {
    state
        .reading
        .begin_fresh(&state.lock().unwrap(), input(aid), fresh)
        .unwrap()
}

#[tokio::test]
async fn shared_translation_survives_restart_and_pause_without_another_paid_attempt() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("shared.sqlite3");
    let state = Application::start(&path, None);
    let (base, worker) = translation_server(json!("Hello house"));
    configure(&state, &base);
    let a = begin(&state, reading::ReadingAid::Translation, false);
    let b = begin(&state, reading::ReadingAid::Translation, false);
    let (first, second) =
        tokio::join!(run_owned_reading(&state, &a), run_owned_reading(&state, &b));
    let first = first.unwrap();
    let second = second.unwrap();
    worker.join().unwrap();
    crate::ai::inspection::verify_workspace_reads(&state.lock().unwrap());
    assert_eq!(first.translation, second.translation);
    assert_eq!(
        first.receipt["response"]["sourceExecutionId"],
        second.receipt["response"]["sourceExecutionId"]
    );
    assert_ne!(first.receipt["attemptId"], second.receipt["attemptId"]);
    assert_eq!(state.lock().unwrap().profile().unwrap().global.attempts, 1);
    assert_eq!(
        state
            .lock()
            .unwrap()
            .connection
            .query_row(
                "SELECT count(*) FROM sqlite_master WHERE name='inference_executions'",
                [],
                |r| r.get::<_, i64>(0)
            )
            .unwrap(),
        0
    );
    let effort =
        crate::learning::effort::read(&state.lock().unwrap().connection, "spanish").unwrap();
    assert_eq!(effort.explorations, 1);
    let presented: Vec<_> = [&first, &second]
        .into_iter()
        .filter_map(|result| result.receipt["effortAward"]["id"].as_str())
        .collect();
    assert_eq!(presented, vec![effort.recent[0].id.as_str()]);
    assert!(effort.recent[0].conversation_id.is_none());
    drop(state);
    let state = Application::start(&path, None);
    state
        .lock()
        .unwrap()
        .connection
        .execute("UPDATE ai_config SET paused=1", [])
        .unwrap();
    let c = begin(&state, reading::ReadingAid::Translation, false);
    let cached = run_owned_reading(&state, &c).await.unwrap();
    assert_eq!(cached.translation, first.translation);
    assert_eq!(cached.receipt["response"]["cacheHit"], true);
    assert!(cached.receipt["effortAward"].is_null());
    assert_eq!(
        crate::learning::effort::read(&state.lock().unwrap().connection, "spanish")
            .unwrap()
            .explorations,
        1
    );
    assert_eq!(state.lock().unwrap().profile().unwrap().global.attempts, 1);
    let retry = begin(&state, reading::ReadingAid::Translation, true);
    assert!(run_owned_reading(&state, &retry).await.is_err());
    let held = crate::ai::workspace_graph::receipt(&state.lock().unwrap().connection, &retry)
        .unwrap()
        .unwrap();
    // A rejected request releases its native consumer; it is not left queued.
    assert_eq!(held["state"], "cancelled");
    assert_eq!(held["dispatched"], false);
    assert_eq!(
        crate::learning::effort::read(&state.lock().unwrap().connection, "spanish")
            .unwrap()
            .explorations,
        1
    );
    assert_eq!(state.lock().unwrap().profile().unwrap().global.attempts, 1);
}

#[tokio::test]
async fn closing_the_initiator_keeps_the_other_consumer_and_shared_receipt_alive() {
    let dir = tempfile::tempdir().unwrap();
    let state = Application::start(&dir.path().join("shared.sqlite3"), None);
    let (started, ready) = tokio::sync::oneshot::channel();
    let (release, released) = std::sync::mpsc::channel();
    let (base, worker) = crate::application::test_server::structured_server(move |source| {
        started.send(()).unwrap();
        released.recv_timeout(Duration::from_secs(5)).unwrap();
        json!({"source":source,"translation":"Hello house"}).to_string()
    });
    configure(&state, &base);
    let a = begin(&state, reading::ReadingAid::Translation, false);
    let b = begin(&state, reading::ReadingAid::Translation, false);
    let cancel = async {
        ready.await.unwrap();
        state.reading.cancel(&state.lock().unwrap(), &a).unwrap();
        tokio::time::sleep(Duration::from_millis(100)).await;
        release.send(()).unwrap();
    };
    let (first, second, _) = tokio::join!(
        run_owned_reading(&state, &a),
        run_owned_reading(&state, &b),
        cancel
    );
    assert!(first.is_err());
    assert_eq!(second.unwrap().translation.as_deref(), Some("Hello house"));
    worker.join().unwrap();
    let store = state.lock().unwrap();
    let receipts = reading::activity(&store).unwrap();
    let closed = receipts.iter().find(|r| r["id"] == a).unwrap();
    assert_eq!(closed["state"], "cancelled");
    assert_eq!(closed["sourceExecution"]["state"], "succeeded");
    assert_eq!(
        closed["sourceExecution"]["response"]["providerId"],
        "structured-receipt"
    );
    assert_eq!(store.profile().unwrap().global.attempts, 1);
}

#[tokio::test]
async fn retry_fills_missing_glosses_preserves_accepted_spans_and_reuse_is_local() {
    let dir = tempfile::tempdir().unwrap();
    let state = Application::start(&dir.path().join("shared.sqlite3"), None);
    let mut call = 0;
    let (base, worker) = structured_sequence(2, move |_| {
        call += 1;
        let first = json!({"first":"g0000","last":"g0003","kind":"gloss","gloss":if call == 1 {"hello"} else {"changed"},"romanization":null,"pronunciation":null});
        let mut spans = vec![first];
        if call == 2 {
            spans.push(json!({"first":"g0005","last":"g0008","kind":"gloss","gloss":"house","romanization":null,"pronunciation":null}));
        }
        json!({"spans":spans}).to_string()
    });
    configure(&state, &base);
    let a = begin(&state, reading::ReadingAid::WordGloss, false);
    let partial = run_owned_reading(&state, &a).await.unwrap();
    assert_eq!(
        partial.gloss.as_ref().unwrap().coverage,
        model::GlossCoverage::Partial
    );
    let b = begin(&state, reading::ReadingAid::WordGloss, false);
    let reused = run_owned_reading(&state, &b).await.unwrap();
    assert_eq!(reused.receipt["response"]["cacheHit"], true);
    let c = begin(&state, reading::ReadingAid::WordGloss, true);
    let repaired = run_owned_reading(&state, &c).await.unwrap();
    let view = repaired.gloss.unwrap();
    assert_eq!(view.coverage, model::GlossCoverage::Complete);
    assert_eq!(view.segments[0].gloss.as_deref(), Some("hello"));
    assert!(
        view.segments
            .iter()
            .any(|s| s.gloss.as_deref() == Some("house"))
    );
    let payloads = worker.join().unwrap();
    assert_ne!(
        payloads[0]["items"][0]["attempt_id"],
        payloads[1]["items"][0]["attempt_id"]
    );
    assert_eq!(state.lock().unwrap().profile().unwrap().global.attempts, 2);
    let query = reading::saved::SavedGlossQuery {
        scope: reading::ReadingScope {
            language: "spanish".into(),
            variety: None,
            explanation: "english".into(),
            explanation_variety: None,
        },
        surfaces: vec!["casa".into()],
    };
    let sources = reading::text_sources::sources(&state.lock().unwrap(), &query).unwrap();
    assert!(sources.iter().any(|source| {
        source
            .segments
            .iter()
            .any(|s| s.gloss.as_deref() == Some("house"))
    }));
    results::set_capacity(&state.lock().unwrap().connection, 0).unwrap();
    assert!(
        reading::text_sources::sources(&state.lock().unwrap(), &query)
            .unwrap()
            .is_empty()
    );
    assert_eq!(state.lock().unwrap().profile().unwrap().global.attempts, 2);
    assert!(
        crate::ai::workspace_graph::receipt(&state.lock().unwrap().connection, &a)
            .unwrap()
            .is_some()
    );
}

#[test]
fn exact_contract_identity_is_independent_of_consumer_but_sensitive_to_inputs() {
    let dir = tempfile::tempdir().unwrap();
    let state = Application::start(&dir.path().join("shared.sqlite3"), None);
    configure(&state, "http://127.0.0.1:1/v1");
    let store = state.lock().unwrap();
    let key = |input| {
        reading::Request::capture(&store, input)
            .unwrap()
            .native_context()
            .unwrap()["sourceKey"]
            .clone()
    };
    let original = input(reading::ReadingAid::Translation);
    assert_eq!(key(original.clone()), key(original.clone()));
    let mut changed = original.clone();
    changed.text.push('.');
    assert_ne!(key(original.clone()), key(changed));
    let mut changed = original.clone();
    changed.aid = reading::ReadingAid::Explanations;
    assert_ne!(key(original.clone()), key(changed));
    let mut changed = original.clone();
    changed.variety = Some("spanish-mexico".into());
    assert_ne!(key(original.clone()), key(changed));
    let before = key(original.clone());
    store
        .connection
        .execute("UPDATE ai_config SET fast_model='different-model'", [])
        .unwrap();
    assert_ne!(before, key(original));
}

#[tokio::test]
async fn an_explicit_retry_does_not_join_work_already_in_flight() {
    let dir = tempfile::tempdir().unwrap();
    let state = Application::start(&dir.path().join("shared.sqlite3"), None);
    let (started, ready) = tokio::sync::oneshot::channel();
    let mut started = Some(started);
    let (release, released) = std::sync::mpsc::channel();
    let (base, worker) = structured_sequence(2, move |source| {
        if let Some(started) = started.take() {
            started.send(()).unwrap();
            released.recv_timeout(Duration::from_secs(5)).unwrap();
        }
        json!({"source":source,"translation":"Hello house"}).to_string()
    });
    configure(&state, &base);
    let a = begin(&state, reading::ReadingAid::Translation, false);
    let fresh = async {
        ready.await.unwrap();
        let b = begin(&state, reading::ReadingAid::Translation, true);
        let release_after_dispatch = async {
            tokio::time::timeout(Duration::from_secs(3), async {
                loop {
                    let count: i64 = state
                        .lock()
                        .unwrap()
                        .connection
                        .query_row(
                            "SELECT count(*) FROM workspace_graph_transport_identities",
                            [],
                            |r| r.get(0),
                        )
                        .unwrap();
                    if count == 2 {
                        break;
                    }
                    tokio::time::sleep(Duration::from_millis(5)).await;
                }
            })
            .await
            .unwrap();
            release.send(()).unwrap();
        };
        let (result, _) = tokio::join!(run_owned_reading(&state, &b), release_after_dispatch);
        result.unwrap()
    };
    let (first, second) = tokio::join!(run_owned_reading(&state, &a), fresh);
    assert_ne!(
        first.unwrap().receipt["response"]["sourceExecutionId"],
        second.receipt["response"]["sourceExecutionId"]
    );
    worker.join().unwrap();
    assert_eq!(state.lock().unwrap().profile().unwrap().global.attempts, 2);
}

#[tokio::test]
async fn submitted_work_settles_after_its_last_card_closes() {
    let dir = tempfile::tempdir().unwrap();
    let state = Application::start(&dir.path().join("shared.sqlite3"), None);
    let (started, ready) = tokio::sync::oneshot::channel();
    let (release, released) = std::sync::mpsc::channel();
    let (base, worker) = crate::application::test_server::structured_server(move |source| {
        started.send(()).unwrap();
        released.recv_timeout(Duration::from_secs(5)).unwrap();
        json!({"source":source,"translation":"Hello house"}).to_string()
    });
    configure(&state, &base);
    let a = begin(&state, reading::ReadingAid::Translation, false);
    let close = async {
        ready.await.unwrap();
        state.reading.cancel(&state.lock().unwrap(), &a).unwrap();
    };
    let (result, _) = tokio::join!(run_owned_reading(&state, &a), close);
    assert!(result.is_err());
    release.send(()).unwrap();
    tokio::time::timeout(Duration::from_secs(3), async {
        loop {
            let receipt =
                crate::ai::workspace_graph::receipt(&state.lock().unwrap().connection, &a)
                    .unwrap()
                    .unwrap();
            if receipt["state"] == "succeeded" {
                break;
            }
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
    })
    .await
    .unwrap();
    worker.join().unwrap();
    let b = begin(&state, reading::ReadingAid::Translation, false);
    assert_eq!(
        run_owned_reading(&state, &b).await.unwrap().receipt["response"]["cacheHit"],
        true
    );
    assert_eq!(state.lock().unwrap().profile().unwrap().global.attempts, 1);
}

#[tokio::test]
async fn cache_publication_failure_retains_provider_usage_and_response_details() {
    let dir = tempfile::tempdir().unwrap();
    let state = Application::start(&dir.path().join("shared.sqlite3"), None);
    let (base, worker) = translation_server(json!("Hello house"));
    configure(&state, &base);
    state.lock().unwrap().connection.execute_batch("CREATE TRIGGER reject_reading_cache BEFORE INSERT ON workspace_reading_cache BEGIN SELECT RAISE(ABORT,'cache publication refused'); END;").unwrap();
    let id = begin(&state, reading::ReadingAid::Translation, false);
    let Err(error) = run_owned_reading(&state, &id).await else {
        panic!("publication must fail explicitly")
    };
    worker.join().unwrap();
    assert_eq!(
        error.diagnostics.as_ref().unwrap()["response"]["providerId"],
        "structured-receipt"
    );
    let store = state.lock().unwrap();
    let receipt = crate::ai::workspace_graph::receipt(&store.connection, &id)
        .unwrap()
        .unwrap();
    assert_eq!(receipt["state"], "succeeded");
    let activity = reading::activity(&store).unwrap();
    assert_eq!(
        activity.iter().find(|row| row["id"] == id).unwrap()["state"],
        "failed"
    );
    assert_eq!(receipt["response"]["providerId"], "structured-receipt");
    assert_eq!(receipt["response"]["inputTokens"], 21);
    assert_eq!(store.profile().unwrap().global.attempts, 1);
    assert_eq!(
        results::settings(&store.connection).unwrap().result_count,
        0
    );
}
