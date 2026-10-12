use super::*;
use rusqlite::OptionalExtension;
use std::io::{Read, Write};
use std::sync::atomic::{AtomicBool, Ordering};

fn request(socket: &mut std::net::TcpStream) -> String {
    socket
        .set_read_timeout(Some(Duration::from_secs(10)))
        .unwrap();
    let mut bytes = Vec::new();
    loop {
        let mut byte = [0];
        socket.read_exact(&mut byte).unwrap();
        bytes.push(byte[0]);
        if bytes.ends_with(b"\r\n\r\n") {
            break;
        }
    }
    let headers = String::from_utf8(bytes).unwrap();
    let length = headers
        .lines()
        .find_map(|line| {
            line.to_lowercase()
                .strip_prefix("content-length: ")
                .and_then(|n| n.parse().ok())
        })
        .unwrap_or(0);
    let mut body = vec![0; length];
    socket.read_exact(&mut body).unwrap();
    headers + &String::from_utf8(body).unwrap()
}

fn respond(socket: &mut std::net::TcpStream, body: serde_json::Value) {
    let body = body.to_string();
    write!(
        socket,
        "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
        body.len(),
        body
    )
    .unwrap();
}

// Exercise real HTTP dispatch and receipt settlement, not just subscription bookkeeping.
#[tokio::test]
async fn invalid_independent_input_fails_before_execution() {
    let directory = tempfile::tempdir().unwrap();
    let state = Application::start(&directory.path().join("speech.sqlite3"), None);
    let target = access::ResolvedTarget {
        audio_resolution: None,
        route: crate::model::ConnectionRoute::Custom,
        revision: 1,
        url: "http://127.0.0.1:1/v1/audio/speech".into(),
        model: "model".into(),
        credential: None,
    };
    let result = state
        .shared_speech(
            target,
            audio::SpeechInput {
                language_tag: "en".into(),
                text: String::new(),
                language: "en".into(),
                voice: "unused".into(),
            },
            "workspace".into(),
            "consumer",
            || Ok(()),
        )
        .await;
    assert!(matches!(result,Err(e) if e.code == ErrorCode::Validation));
    let count: i64 = state
        .lock()
        .unwrap()
        .connection
        .query_row("SELECT count(*) FROM workspace_graph_consumers", [], |r| {
            r.get(0)
        })
        .unwrap();
    assert_eq!(count, 0);
}

#[tokio::test]
async fn cancelling_one_consumer_preserves_shared_dispatch_and_receipt() {
    cancellation(false, false, false).await;
}

#[tokio::test]
async fn paused_speech_has_an_undispatched_shared_receipt() {
    let directory = tempfile::tempdir().unwrap();
    let state = Application::start(&directory.path().join("speech.sqlite3"), None);
    let (target, install) = {
        let store = state.lock().unwrap();
        store.connection.execute("UPDATE ai_config SET route='custom',paused=1,custom_config=json_set(custom_config,'$.baseUrl','http://127.0.0.1:1/v1','$.bearerAuth',json('false'))", []).unwrap();
        (
            access::resolve(&store.connection, access::Capability::Speech).unwrap(),
            store.snapshot().unwrap().learner.id,
        )
    };
    assert!(
        state
            .shared_speech(
                target,
                audio::SpeechInput {
                    language_tag: "en".into(),
                    text: "PRIVATE-UNSENT".into(),
                    language: "en".into(),
                    voice: "unused".into(),
                },
                install,
                "paused-consumer",
                || Ok(())
            )
            .await
            .is_err()
    );
    let store = state.lock().unwrap();
    let receipt = results::receipt_for_consumer(&store.connection, "paused-consumer")
        .unwrap()
        .unwrap();
    assert_eq!(receipt["state"], "cancelled");
    assert_eq!(receipt["dispatched"], false);
    assert!(!receipt.to_string().contains("PRIVATE-UNSENT"));
    assert_eq!(store.profile().unwrap().global.attempts, 0);
}

#[tokio::test]
async fn cancelling_all_consumers_after_dispatch_still_settles_receipt() {
    cancellation(true, false, false).await;
}

#[tokio::test]
async fn failed_audio_cache_write_preserves_provider_receipt_and_usage() {
    cancellation(false, true, false).await;
}

#[tokio::test]
async fn streaming_consumers_share_early_audio_and_cancel_independently() {
    cancellation(false, false, true).await;
}

#[tokio::test]
async fn revoking_stream_authority_retains_redacted_partial_receipt_without_caching() {
    let directory = tempfile::tempdir().unwrap();
    let state = Application::start(&directory.path().join("speech.sqlite3"), None);
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let (target, install) = {
        let store = state.lock().unwrap();
        let base = format!("http://{}/v1", listener.local_addr().unwrap());
        store.connection.execute("UPDATE ai_config SET route='custom',custom_config=json_set(custom_config,'$.baseUrl',?1,'$.bearerAuth',json('false'))",[base]).unwrap();
        (
            access::resolve(&store.connection, access::Capability::Speech).unwrap(),
            store.snapshot().unwrap().learner.id,
        )
    };
    let worker = std::thread::spawn(move || {
        let (mut socket, _) = listener.accept().unwrap();
        request(&mut socket);
        let prefix = format!(
            "{}\n{}\n",
            json!({"version":3,"seq":0,"type":"start","format":"pcm_s16le","sample_rate":24000,"channels":1}),
            json!({"version":3,"seq":1,"type":"audio","response":{"audio_base64":"ZAA=","alignment":null},
                "receipt":{"request_id":"partial-request","diagnostics":{"code":"fixture_error","reason":"PRIVATE-SOURCE","authorization":"Bearer hidden-token"}}})
        );
        write!(socket,"HTTP/1.1 200 OK\r\nContent-Type: application/x-ndjson\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",prefix.len()+1,prefix).unwrap();
        socket.flush().unwrap();
        let mut byte = [0];
        // Revocation must close the response without asking for another generation.
        assert_eq!(socket.read(&mut byte).unwrap(), 0);
    });
    let request = state.shared_speech(
        target,
        audio::SpeechInput {
            text: "PRIVATE-SOURCE".into(),
            language_tag: "en".into(),
            language: "English".into(),
            voice: "unused".into(),
        },
        install,
        "consumer",
        || Ok(()),
    );
    let revoke = async {
        loop {
            let received = {
                let store = state.lock().unwrap();
                let id: Option<String> = store
                    .connection
                    .query_row(
                        "SELECT stream_id FROM workspace_graph_consumers WHERE consumer_id='consumer'",
                        [],
                        |r| r.get::<_, Option<String>>(0),
                    )
                    .optional()
                    .unwrap().flatten();
                id.is_some_and(|id| {
                    state
                        .speech_streams
                        .lock()
                        .unwrap()
                        .read(&id, 0)
                        .unwrap()
                        .is_some()
                })
            };
            if received {
                break;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
        state
            .lock()
            .unwrap()
            .connection
            .execute("UPDATE ai_config SET paused=1", [])
            .unwrap();
    };
    let (result, _) = tokio::time::timeout(Duration::from_secs(5), async {
        tokio::join!(request, revoke)
    })
    .await
    .unwrap();
    assert!(result.is_err());
    worker.join().unwrap();
    let store = state.lock().unwrap();
    assert!(
        results::for_consumer(&store.connection, "consumer")
            .unwrap()
            .is_none()
    );
    let receipt = results::receipt_for_consumer(&store.connection, "consumer")
        .unwrap()
        .unwrap();
    assert_eq!(receipt["response"]["providerId"], "partial-request");
    let details = receipt.to_string();
    assert!(details.contains("fixture_error"));
    assert!(!details.contains("PRIVATE-SOURCE"));
    assert!(!details.contains("hidden-token"));
}

async fn cancellation(cancel_all: bool, reject_blob: bool, streamed: bool) {
    let directory = tempfile::tempdir().unwrap();
    let state = Application::start(&directory.path().join("speech.sqlite3"), None);
    // Joining pending work remains available when retained reuse is disabled.
    if !cancel_all && !reject_blob {
        results::set_capacity(&state.lock().unwrap().connection, 0).unwrap();
    }
    // Fail derived audio publication after transport capture, not the new
    // original request write that must succeed before any provider dispatch.
    if reject_blob {
        state.lock().unwrap().connection.execute_batch("CREATE TRIGGER reject_speech_cache BEFORE INSERT ON workspace_graph_audio_cache BEGIN SELECT RAISE(ABORT,'fixture cache failure'); END").unwrap();
    }
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let (target, install) = {
        let store = state.lock().unwrap();
        let base = format!("http://{}/v1", listener.local_addr().unwrap());
        store.connection.execute("UPDATE ai_config SET route='custom',custom_config=json_set(custom_config,'$.baseUrl',?1,'$.bearerAuth',json('false'))", [&base]).unwrap();
        (
            access::resolve(&store.connection, access::Capability::Speech).unwrap(),
            store
                .connection
                .query_row("SELECT id FROM learner LIMIT 1", [], |r| {
                    r.get::<_, String>(0)
                })
                .unwrap(),
        )
    };
    let model = target.model.clone();
    let (started, dispatched) = tokio::sync::oneshot::channel();
    let (release, released) = std::sync::mpsc::channel();
    let worker = std::thread::spawn(move || {
        let (mut socket, _) = listener.accept().unwrap();
        let wire = request(&mut socket);
        assert!(wire.starts_with("POST /v1/audio/speech "));
        let body: serde_json::Value =
            serde_json::from_str(wire.split_once("\r\n\r\n").unwrap().1).unwrap();
        assert_eq!(body.as_object().unwrap().len(), 3);
        assert!(body["language_code"].is_null());
        let chunks = if streamed { 80 } else { 1 };
        let terminal = format!(
            "{}\n",
            json!({"version":3,"seq":chunks+1,"type":"complete","total_samples":chunks,"alignment":null,
            "usage":{"requested_model":model,"actual_model":model,"request_id":"shared-request","cost_micros":null}})
        );
        if streamed {
            // More audio chunks than the evidence limit: unchanged response
            // metadata must not be counted as another observation per chunk.
            let mut prefix = format!(
                "{}\n",
                json!({"version":3,"seq":0,"type":"start","format":"pcm_s16le","sample_rate":24000,"channels":1})
            );
            for sequence in 1..=chunks {
                prefix.push_str(&format!("{}\n",json!({"version":3,"seq":sequence,"type":"audio","response":{"audio_base64":"ZAA=","alignment":null},"receipt":{"request_id":"shared-request"}})));
            }
            write!(socket,"HTTP/1.1 200 OK\r\nContent-Type: application/x-ndjson\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",prefix.len()+terminal.len(),prefix).unwrap();
            socket.flush().unwrap();
        }
        started.send(()).unwrap();
        released.recv_timeout(Duration::from_secs(10)).unwrap();
        if streamed {
            socket.write_all(terminal.as_bytes()).unwrap();
            return;
        }
        respond(
            &mut socket,
            json!({"version":3,"response":{"audio_base64":"ZAA="},
            "usage":{"requested_model":model,"actual_model":model,"provider":"elevenlabs","request_id":"shared-request","cost_micros":null,"allowance_micros":12}}),
        );
    });
    let active = AtomicBool::new(true);
    let input = audio::SpeechInput {
        language_tag: "en".into(),
        text: "shared source".into(),
        language: "en".into(),
        voice: "unused".into(),
    };
    let first = state.shared_speech(
        target.clone(),
        input.clone(),
        install.clone(),
        "first",
        || {
            if active.load(Ordering::SeqCst) {
                Ok(())
            } else {
                Err(AppError::new(ErrorCode::Conflict, "Consumer closed."))
            }
        },
    );
    let second = state.shared_speech(target, input, install, "second", || {
        if !cancel_all || active.load(Ordering::SeqCst) {
            Ok(())
        } else {
            Err(AppError::new(ErrorCode::Conflict, "Consumer closed."))
        }
    });
    let controller = async {
        tokio::time::timeout(Duration::from_secs(5), dispatched)
            .await
            .unwrap()
            .unwrap();
        // Both futures have subscribed before the async HTTP response arrives.
        let count: i64 = state
            .lock()
            .unwrap()
            .connection
            .query_row("SELECT count(*) FROM workspace_graph_consumers", [], |r| {
                r.get(0)
            })
            .unwrap();
        assert_eq!(count, 2);
        if streamed {
            tokio::time::timeout(Duration::from_secs(5), async {
                loop {
                    let id: String = state.lock().unwrap().connection.query_row("SELECT stream_id FROM workspace_graph_consumers WHERE consumer_id='first'", [], |r|r.get(0)).unwrap();
                    if state.speech_streams.lock().unwrap().read(&id,0).unwrap().is_some_and(|pcm|pcm.starts_with(&[100,0])) { break; }
                    tokio::time::sleep(Duration::from_millis(10)).await;
                }
            }).await.unwrap();
            assert!(
                results::for_consumer(&state.lock().unwrap().connection, "second")
                    .unwrap()
                    .is_none()
            );
        }
        active.store(false, Ordering::SeqCst);
    };
    let first_then_release = async {
        let result = first.await;
        assert!(matches!(result, Err(ref e) if e.code == ErrorCode::Conflict));
        if !cancel_all {
            release.send(()).unwrap();
        }
    };
    let second_then_release = async {
        let result = second.await;
        if cancel_all {
            assert!(matches!(result, Err(ref e) if e.code == ErrorCode::Conflict));
            release.send(()).unwrap();
        }
        result
    };
    let (_, saved, _) = tokio::time::timeout(Duration::from_secs(10), async {
        tokio::join!(first_then_release, second_then_release, controller)
    })
    .await
    .unwrap();
    worker.join().unwrap();
    tokio::time::timeout(Duration::from_secs(5), async {
        loop {
            let settled = results::receipt_for_consumer(&state.lock().unwrap().connection, "first")
                .unwrap()
                .is_some_and(|r| r["state"] == if reject_blob { "failed" } else { "succeeded" });
            if settled {
                break;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .unwrap();
    let store = state.lock().unwrap();
    let receipt = results::receipt_for_consumer(&store.connection, "first")
        .unwrap()
        .unwrap();
    if reject_blob {
        let error = saved.err().unwrap();
        assert_eq!(error.code, ErrorCode::Storage);
        assert_eq!(
            error.diagnostics.unwrap()["response"]["providerId"],
            "shared-request"
        );
        assert!(
            results::for_consumer(&store.connection, "second")
                .unwrap()
                .is_none()
        );
        assert_eq!(receipt["state"], "failed");
    } else if !cancel_all {
        assert!(saved.unwrap().execution.starts_with("graph-audio:"));
    }
    assert_eq!(
        receipt["state"],
        if reject_blob { "failed" } else { "succeeded" }
    );
    assert_eq!(receipt["response"]["providerId"], "shared-request");
    assert!(receipt["response"]["costMicros"].is_null());
    assert!(!receipt.to_string().contains("shared source"));
    assert_eq!(store.profile().unwrap().global.attempts, 1);
    results::set_capacity(&store.connection, 0).unwrap();
    assert!(
        results::for_consumer(&store.connection, "first")
            .unwrap()
            .is_none()
    );
    assert_eq!(
        results::receipt_for_consumer(&store.connection, "first")
            .unwrap()
            .unwrap(),
        receipt
    );
}

#[tokio::test]
async fn native_playback_reuses_cache_across_restart_and_only_regenerates_when_requested() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("native-speech.sqlite3");
    let state = Application::start(&path, None);
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let (target, install) = {
        let store = state.lock().unwrap();
        store.connection.execute("UPDATE ai_config SET route='custom',custom_config=json_set(custom_config,'$.baseUrl',?1,'$.bearerAuth',json('false'))",[format!("http://{}/v1",listener.local_addr().unwrap())]).unwrap();
        (
            access::resolve(&store.connection, access::Capability::Speech).unwrap(),
            store.snapshot().unwrap().learner.id,
        )
    };
    let worker = std::thread::spawn(move || {
        for _ in 0..2 {
            let (mut socket, _) = listener.accept().unwrap();
            request(&mut socket);
            respond(
                &mut socket,
                json!({"version":3,"response":{"audio_base64":"ZAA="},"usage":{"request_id":"native-audio-request"}}),
            );
        }
    });
    let input = audio::SpeechInput {
        text: "¿Qué? 日本語 e\u{301}".into(),
        language_tag: "es".into(),
        language: "Spanish".into(),
        voice: "unused".into(),
    };
    let first = state
        .shared_speech(
            target.clone(),
            input.clone(),
            install.clone(),
            "one",
            || Ok(()),
        )
        .await
        .unwrap();
    assert!(!first.cached);
    let second = state
        .shared_speech(
            target.clone(),
            input.clone(),
            install.clone(),
            "two",
            || Ok(()),
        )
        .await
        .unwrap();
    assert!(second.cached);
    assert_eq!(first.execution, second.execution);
    drop(state);
    let state = Application::start(&path, None);
    let third = state
        .shared_speech(
            target.clone(),
            input.clone(),
            install.clone(),
            "three",
            || Ok(()),
        )
        .await
        .unwrap();
    crate::ai::inspection::verify_workspace_reads(&state.lock().unwrap());
    assert!(third.cached);
    assert_eq!(first.execution, third.execution);
    results::set_capacity(&state.lock().unwrap().connection, 0).unwrap();
    let fourth = state
        .shared_speech(target, input, install, "four", || Ok(()))
        .await
        .unwrap();
    assert!(!fourth.cached);
    assert_ne!(first.execution, fourth.execution);
    worker.join().unwrap();
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
            .query_row("SELECT count(*) FROM workspace_graph_runs", [], |r| r
                .get::<_, i64>(0))
            .unwrap(),
        4
    );
    assert_eq!(
        store
            .connection
            .query_row(
                "SELECT count(*) FROM workspace_graph_audio_receipts",
                [],
                |r| r.get::<_, i64>(0)
            )
            .unwrap(),
        2
    );
}
