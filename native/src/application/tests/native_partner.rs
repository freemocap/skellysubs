//! Command -> application scheduler -> shared HTTP transport -> product state.
use super::*;
use crate::model::Action;
use tokio::io::{AsyncReadExt, AsyncWriteExt};

#[tokio::test]
async fn native_partner_application_host_publishes_reply_and_requested_translation() {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let base = format!("http://{}/v1", listener.local_addr().unwrap());
    let server = tokio::spawn(async move {
        let mut completed = 0;
        let mut held_feedback = None;
        let mut reply_sent = false;
        while completed < 3 {
            let (mut socket, _) =
                tokio::time::timeout(std::time::Duration::from_secs(30), listener.accept())
                    .await
                    .unwrap()
                    .unwrap();
            let mut bytes = Vec::new();
            let (start, length) = loop {
                let mut chunk = [0; 4096];
                let count = socket.read(&mut chunk).await.unwrap();
                assert!(count > 0);
                bytes.extend_from_slice(&chunk[..count]);
                if let Some(end) = bytes.windows(4).position(|w| w == b"\r\n\r\n") {
                    let headers = String::from_utf8_lossy(&bytes[..end]);
                    let length = headers
                        .lines()
                        .find_map(|line| {
                            line.to_ascii_lowercase()
                                .strip_prefix("content-length:")
                                .map(|v| v.trim().parse::<usize>().unwrap())
                        })
                        .unwrap_or(0);
                    if bytes.len() >= end + 4 + length {
                        break (end + 4, length);
                    }
                }
            };
            let mut feedback = false;
            let mut reply = false;
            let (content_type, body) = if bytes.starts_with(b"GET /v1/protocol ") {
                assert!(bytes.starts_with(b"GET /v1/protocol "));
                (
                    "application/json",
                    "{\"operations_versions\":[3]}".to_string(),
                )
            } else {
                assert!(bytes.starts_with(b"POST /v1/operations "));
                let payload: serde_json::Value =
                    serde_json::from_slice(&bytes[start..start + length]).unwrap();
                let item = &payload["items"][0];
                completed += 1;
                let properties =
                    &item["request"]["response_format"]["json_schema"]["schema"]["properties"];
                let content = if properties.get("translation").is_some() {
                    let messages = item["request"]["messages"].as_array().unwrap();
                    serde_json::json!({"source":messages.last().unwrap()["content"],"translation":"Hello."}).to_string()
                } else if properties.get("meaning_recovered").is_some() {
                    feedback = true;
                    serde_json::json!({"meaning_recovered":"full","items":[]}).to_string()
                } else {
                    assert!(item["request"]["response_format"].is_null());
                    reply = true;
                    "Hola.".to_string()
                };
                (
                    "application/x-ndjson",
                    format!(
                        "{}\n{}\n",
                        serde_json::json!({"type":"result","operation_id":item["operation_id"],"attempt_id":item["attempt_id"],"response":{"id":"native-host-receipt","model":"fixture-model","choices":[{"finish_reason":"stop","message":{"content":content}}],"usage":{"prompt_tokens":21,"completion_tokens":4}}}),
                        serde_json::json!({"type":"complete","count":1})
                    ),
                )
            };
            let response = format!(
                "HTTP/1.1 200 OK\r\nContent-Type: {content_type}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                body.len()
            );
            if feedback && !reply_sent {
                // Do not complete feedback until the independent reply request arrives.
                // Serial provider admission deadlocks here and fails the timeout.
                held_feedback = Some((socket, response));
                continue;
            }
            socket.write_all(response.as_bytes()).await.unwrap();
            if reply {
                reply_sent = true;
                if let Some((mut socket, response)) = held_feedback.take() {
                    socket.write_all(response.as_bytes()).await.unwrap();
                }
            }
        }
    });
    let dir = tempfile::tempdir().unwrap();
    let app = Application::start(&dir.path().join("native-host.sqlite3"), None);
    let conversation = {
        let mut store = app.lock().unwrap();
        store.prepare_chat().unwrap();
        store.connection.execute("UPDATE ai_config SET route='custom',custom_config=json_set(custom_config,'$.baseUrl',?1,'$.bearerAuth',json('false'))",[base]).unwrap();
        store.connection.execute("UPDATE learner SET preferences=json_set(preferences,'$.execution.reading','on_demand','$.execution.assessment','on_demand','$.execution.replyBrief','on_demand')",[]).unwrap();
        let conversation = store.snapshot().unwrap().conversations[0].clone();
        store.connection.execute("UPDATE conversation_settings SET settings=json_set(settings,'$.readAloud',json('false')) WHERE conversation_id=?1",[&conversation.id]).unwrap();
        let session_id = store.session_id.clone();
        store
            .execute(Command {
                session_id,
                action_id: uuid::Uuid::new_v4().to_string(),
                action: Action::SendMessage {
                    conversation_id: conversation.id.clone(),
                    expected_revision: conversation.revision,
                    text: "Hola".into(),
                    input: Default::default(),
                },
            })
            .unwrap();
        conversation.id
    };
    let client = provider::client().unwrap();
    for translation in [false, true] {
        let deadline = tokio::time::Instant::now() + std::time::Duration::from_secs(30);
        loop {
            let completed = app.scheduler_wake.notified();
            tokio::pin!(completed);
            completed.as_mut().enable();
            schedule(&app, &client).await.unwrap();
            let ready = {
                let store = app.lock().unwrap();
                let product = store.conversation_snapshot(&conversation, None).unwrap();
                product.messages.iter().any(|m| {
                    m.role == "assistant"
                        && if translation {
                            m.translation_state.as_deref() == Some("succeeded")
                        } else {
                            m.text == "Hola."
                        }
                })
            };
            if ready {
                break;
            }
            if tokio::time::Instant::now() >= deadline {
                let store = app.lock().unwrap();
                let product = store.conversation_snapshot(&conversation, None).unwrap();
                let turn = &product.turns[0].id;
                let inspection = store
                    .graph_runtime
                    .inspection(&store.connection, &conversation, turn)
                    .unwrap();
                panic!(
                    "native application host did not publish (translation={translation}): {}",
                    serde_json::to_string(&inspection).unwrap()
                );
            }
            tokio::time::timeout_at(deadline, completed)
                .await
                .expect("graph completion did not wake the application host");
        }
        if !translation {
            let mut store = app.lock().unwrap();
            let product = store.conversation_snapshot(&conversation, None).unwrap();
            let message_id = product
                .messages
                .iter()
                .find(|m| m.role == "assistant")
                .unwrap()
                .id
                .clone();
            let session_id = store.session_id.clone();
            store
                .execute(Command {
                    session_id,
                    action_id: uuid::Uuid::new_v4().to_string(),
                    action: Action::RequestMessageHelp {
                        message_id,
                        help: execution::MessageHelp::Translation,
                        retry: false,
                    },
                })
                .unwrap();
        }
    }
    server.await.unwrap();
    let store = app.lock().unwrap();
    assert_eq!(
        store
            .connection
            .query_row("SELECT count(*) FROM graph_transport_identities", [], |r| r
                .get::<_, i64>(0))
            .unwrap(),
        3
    );
}
