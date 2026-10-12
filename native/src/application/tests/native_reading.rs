//! Real reading commands must enter native execution, including template validation.
use super::*;
use crate::application::test_server::structured_server;
use serde_json::json;

#[tokio::test]
async fn native_sentence_completion_validates_source_before_product_publication() {
    for valid in [true, false] {
        let directory = tempfile::tempdir().unwrap();
        let state = Application::start(&directory.path().join("reading.sqlite3"), None);
        let (base, worker) = structured_server(move |_| {
            let examples = if valid {
                ["Yo leo", "Yo corro"]
            } else {
                ["Changed source", "Yo corro"]
            };
            json!({"cards":examples.map(|example| json!({"quote":"Yo ___","title":"Choice","body":"Explanation","example":example,"contrast":""}))}).to_string()
        });
        state.lock().unwrap().connection.execute("UPDATE ai_config SET route='custom',custom_config=json_set(custom_config,'$.baseUrl',?1,'$.bearerAuth',json('false'))", [&base]).unwrap();
        let id = state
            .reading
            .begin(
                &state.lock().unwrap(),
                reading::ReadingInput {
                    conversation_id: None,
                    reference_item: None,
                    text: "Yo ___".into(),
                    language: "spanish".into(),
                    variety: None,
                    explanation: "english".into(),
                    explanation_variety: None,
                    aid: reading::ReadingAid::Completions,
                },
            )
            .unwrap();
        let outcome = run_owned_reading(&state, &id).await;
        let payload = worker.join().unwrap();
        assert_eq!(
            payload["items"][0]["request"]["response_format"]["json_schema"]["schema"]["properties"]
                ["cards"]["minItems"],
            2
        );
        assert_eq!(outcome.is_ok(), valid);
        let store = state.lock().unwrap();
        assert_eq!(
            store
                .connection
                .query_row(
                    "SELECT count(*) FROM sqlite_master WHERE name='inference_executions'",
                    [],
                    |r| r.get::<_, i64>(0)
                )
                .unwrap(),
            0
        );
        assert_eq!(
            store
                .connection
                .query_row(
                    "SELECT count(*) FROM workspace_graph_transport_identities",
                    [],
                    |r| r.get::<_, i64>(0)
                )
                .unwrap(),
            1
        );
        let receipt = crate::ai::workspace_graph::receipt(&store.connection, &id)
            .unwrap()
            .unwrap();
        assert_eq!(receipt["state"], if valid { "succeeded" } else { "failed" });
        assert_eq!(receipt["response"]["providerId"], "structured-receipt");
        if valid {
            assert_eq!(outcome.unwrap().explanations.unwrap().cards.len(), 2);
        } else {
            assert_eq!(
                crate::ai::results::settings(&store.connection)
                    .unwrap()
                    .result_count,
                0
            );
        }
    }
}

#[tokio::test]
async fn native_explanations_reject_invalid_results_and_do_not_publish_to_closed_cards() {
    for cancel in [false, true] {
        let directory = tempfile::tempdir().unwrap();
        let state = Application::start(&directory.path().join("explanations.sqlite3"), None);
        let (started, ready) = tokio::sync::oneshot::channel();
        let (release, released) = std::sync::mpsc::channel();
        let (base, worker) = structured_server(move |_| {
            started.send(()).unwrap();
            released.recv_timeout(Duration::from_secs(5)).unwrap();
            if cancel {
                json!({"cards":[{"quote":"Hola","title":"Greeting","body":"Hello.","example":"Hola.","contrast":""}]}).to_string()
            } else {
                json!({"cards":"invalid"}).to_string()
            }
        });
        state.lock().unwrap().connection.execute("UPDATE ai_config SET route='custom',custom_config=json_set(custom_config,'$.baseUrl',?1,'$.bearerAuth',json('false'))",[base]).unwrap();
        let id = state
            .reading
            .begin(
                &state.lock().unwrap(),
                reading::ReadingInput {
                    conversation_id: None,
                    reference_item: None,
                    text: "Hola".into(),
                    language: "spanish".into(),
                    variety: None,
                    explanation: "english".into(),
                    explanation_variety: None,
                    aid: reading::ReadingAid::Explanations,
                },
            )
            .unwrap();
        let control = async {
            ready.await.unwrap();
            if cancel {
                state.reading.cancel(&state.lock().unwrap(), &id).unwrap();
            }
            release.send(()).unwrap();
        };
        let (result, _) = tokio::join!(run_owned_reading(&state, &id), control);
        assert!(result.is_err());
        worker.join().unwrap();
        tokio::time::timeout(Duration::from_secs(5), async {
            loop {
                let receipt =
                    crate::ai::workspace_graph::receipt(&state.lock().unwrap().connection, &id)
                        .unwrap()
                        .unwrap();
                if receipt["state"] == if cancel { "succeeded" } else { "failed" } {
                    break;
                }
                tokio::time::sleep(Duration::from_millis(10)).await;
            }
        })
        .await
        .unwrap();
        let store = state.lock().unwrap();
        let activity = reading::activity(&store).unwrap();
        let receipt = activity.iter().find(|r| r["id"] == id).unwrap();
        assert_eq!(
            receipt["state"],
            if cancel { "cancelled" } else { "failed" }
        );
        assert!(receipt["effortAward"].is_null());
        assert_eq!(
            receipt["sourceExecution"]["response"]["providerId"],
            "structured-receipt"
        );
        assert_eq!(
            store
                .connection
                .query_row(
                    "SELECT count(*) FROM sqlite_master WHERE name='inference_executions'",
                    [],
                    |r| r.get::<_, i64>(0)
                )
                .unwrap(),
            0
        );
    }
}
