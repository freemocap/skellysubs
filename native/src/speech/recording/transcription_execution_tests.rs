use super::*;
use crate::ai::{
    audio::{TranscriptionRequest, TranscriptionResult},
    results,
};
use serde_json::json;
use std::{
    io::{Read, Write},
    sync::atomic::{AtomicBool, Ordering},
    time::Duration,
};

// Per-phase hang guard for real HTTP, audio analysis and durable graph work on
// contended CI runners. These behavior tests do not impose a latency target.
const FIXTURE_TIMEOUT: Duration = Duration::from_secs(30);

fn server(
    body: serde_json::Value,
) -> (
    String,
    impl std::future::Future<Output = std::result::Result<(), tokio::sync::oneshot::error::RecvError>>,
    std::sync::mpsc::Sender<()>,
    std::thread::JoinHandle<Vec<u8>>,
) {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    server_on(listener, body)
}

fn server_on(
    listener: std::net::TcpListener,
    body: serde_json::Value,
) -> (
    String,
    impl std::future::Future<Output = std::result::Result<(), tokio::sync::oneshot::error::RecvError>>,
    std::sync::mpsc::Sender<()>,
    std::thread::JoinHandle<Vec<u8>>,
) {
    let url = format!("http://{}/v1", listener.local_addr().unwrap());
    let (sent, received) = tokio::sync::oneshot::channel();
    let (release, gate) = std::sync::mpsc::channel();
    let (start, started) = std::sync::mpsc::channel();
    let worker = std::thread::spawn(move || {
        // The caller polls `ready` alongside transcription, after workspace setup.
        // Keep setup time out of the bounded wait for an actual request.
        if started.recv().is_err() {
            return Vec::new();
        }
        listener.set_nonblocking(true).unwrap();
        // Polling transcription still precedes audio inspection and durable
        // admission. This is a hang guard, not a five-second performance test;
        // allow those operations to finish on contended CI workers.
        let deadline = std::time::Instant::now() + FIXTURE_TIMEOUT;
        let (mut socket, _) = loop {
            match listener.accept() {
                Ok(connection) => break connection,
                Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                    assert!(
                        std::time::Instant::now() < deadline,
                        "No transcription request arrived within {FIXTURE_TIMEOUT:?}"
                    );
                    std::thread::sleep(Duration::from_millis(10));
                }
                Err(error) => panic!("Could not accept transcription request: {error}"),
            }
        };
        socket.set_nonblocking(false).unwrap();
        socket.set_read_timeout(Some(FIXTURE_TIMEOUT)).unwrap();
        socket.set_write_timeout(Some(FIXTURE_TIMEOUT)).unwrap();
        let mut request = Vec::new();
        loop {
            let mut buffer = [0; 4096];
            let count = socket.read(&mut buffer).unwrap();
            assert!(count > 0);
            request.extend_from_slice(&buffer[..count]);
            if let Some(end) = request.windows(4).position(|w| w == b"\r\n\r\n") {
                let headers = String::from_utf8_lossy(&request[..end]).to_lowercase();
                assert!(headers.starts_with("post /v1/audio/transcriptions "));
                let length: usize = headers
                    .lines()
                    .find_map(|l| l.strip_prefix("content-length: "))
                    .unwrap()
                    .parse()
                    .unwrap();
                if request.len() >= end + 4 + length {
                    break;
                }
            }
        }
        sent.send(()).unwrap();
        gate.recv_timeout(FIXTURE_TIMEOUT)
            .expect("Test did not release the transcription response");
        let body = body.to_string();
        write!(socket,"HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",body.len(),body).unwrap();
        request
    });
    let ready = async move {
        start.send(()).unwrap();
        received.await
    };
    (url, ready, release, worker)
}
fn reply() -> serde_json::Value {
    json!({"version":3,"response":{"text":"Hola","request_id":"recognition-fixture","duration":1.0,"words":[{"word":"Hola","start":0.1,"end":0.6}],"segments":[{"avg_logprob":-0.05,"no_speech_prob":0.01}]}})
}

#[tokio::test]
async fn mock_request_deadline_excludes_fixture_setup() {
    let (url, ready, release, worker) = server(reply());
    // Workspace initialization can exceed the request timeout on CI runners.
    tokio::time::sleep(Duration::from_millis(5100)).await;
    assert!(
        !worker.is_finished(),
        "Server timed out during fixture setup"
    );
    let request = async {
        let mut socket = std::net::TcpStream::connect(
            url.strip_prefix("http://")
                .unwrap()
                .strip_suffix("/v1")
                .unwrap(),
        )
        .unwrap();
        socket
            .write_all(b"POST /v1/audio/transcriptions HTTP/1.1\r\nContent-Length: 0\r\n\r\n")
            .unwrap();
        ready.await.unwrap();
        release.send(()).unwrap();
        worker.join().unwrap()
    };
    assert!(request.await.starts_with(b"POST /v1/audio/transcriptions "));
}

#[tokio::test]
async fn mock_request_allows_slow_pre_dispatch_work() {
    let (url, ready, release, worker) = server(reply());
    let request = async {
        // Audio inspection and durable admission happen after transcription is
        // polled but before HTTP dispatch. They can exceed five seconds on CI.
        tokio::time::sleep(Duration::from_millis(5100)).await;
        let mut socket = std::net::TcpStream::connect(
            url.strip_prefix("http://")
                .unwrap()
                .strip_suffix("/v1")
                .unwrap(),
        )
        .unwrap();
        socket
            .write_all(b"POST /v1/audio/transcriptions HTTP/1.1\r\nContent-Length: 0\r\n\r\n")
            .unwrap();
        socket
    };
    let gate = async {
        ready.await.unwrap();
        release.send(()).unwrap();
    };
    let (_, _socket) = tokio::join!(gate, request);
    assert!(
        worker
            .join()
            .unwrap()
            .starts_with(b"POST /v1/audio/transcriptions ")
    );
}
fn setup(
    url: &str,
) -> (
    tempfile::TempDir,
    Arc<Application>,
    Transcription,
    TranscriptionRequest,
) {
    let dir = tempfile::tempdir().unwrap();
    let app = Application::recording_fixture(&dir.path().join("transcription.sqlite3"));
    let mut store = app.lock().unwrap();
    store.connection.execute("UPDATE ai_config SET route='custom',custom_config=json_set(custom_config,'$.baseUrl',?1,'$.bearerAuth',json('false'))",[url]).unwrap();
    let item = store
        .create_drill_item(crate::drill::DrillItemInput {
            text: "Hola".into(),
            language: "spanish".into(),
            variety: None,
            explanation: "english".into(),
            explanation_variety: None,
        })
        .unwrap();
    let session = store.start_drill_session("spanish").unwrap();
    let visit = store.enter_drill_visit(&session, &item.id).unwrap();
    let owner = RecordingOwner::DrillItem(item.id);
    let scope = owner.scope(&store).unwrap();
    let recording = Transcription {
        id: "take-1".into(),
        install: store.snapshot().unwrap().learner.id,
        owner,
        visit: Some(visit),
        target: access::resolve(&store.connection, access::Capability::Transcription).unwrap(),
        language: scope.language,
        context: scope.context,
    };
    let wav = super::super::wav::encode_wav(
        &(0..8000)
            .map(|i| ((i as f32) * 0.1).sin() * 0.2)
            .collect::<Vec<_>>(),
        8000,
    )
    .unwrap();
    let input = TranscriptionRequest {
        wav,
        language: recording.language.clone(),
        context: recording.context.clone(),
    };
    drop(store);
    app.failed_take.lock().unwrap().start(&recording.id);
    (dir, app, recording, input)
}

#[tokio::test]
async fn chat_and_drill_share_recognition_but_publish_independently_and_reuse_after_restart() {
    // A real decimal timestamp that an approximate JSON reader changes by one
    // ULP. It must survive graph settlement, publication, cache and restart.
    let end = 0.9163588435374149;
    let mut response = reply();
    response["response"]["words"][0]["end"] = json!(end);
    let (url, ready, release, worker) = server(response);
    let (dir, app, drill, input) = setup(&url);
    let mut chat = drill.clone();
    chat.id = "chat-take".into();
    chat.visit = None;
    {
        let mut store = app.lock().unwrap();
        store.prepare_chat().unwrap();
        chat.owner =
            RecordingOwner::Conversation(store.snapshot().unwrap().conversations[0].id.clone());
    }
    let gate = async {
        ready.await.unwrap();
        release.send(()).unwrap();
    };
    let (a, b, _) = tokio::join!(
        transcribe(app.clone(), drill.clone(), input.wav.clone()),
        transcribe(app.clone(), chat.clone(), input.wav.clone()),
        gate
    );
    let a = a.unwrap();
    let b = b.unwrap();
    crate::ai::inspection::verify_workspace_reads(&app.lock().unwrap());
    assert_eq!(a.text, "Hola");
    assert_eq!(a.text, b.text);
    assert_eq!(
        a.diagnostics.as_ref().unwrap()["sourceExecutionId"],
        b.diagnostics.as_ref().unwrap()["sourceExecutionId"]
    );
    assert!(
        a.diagnostics
            .as_ref()
            .unwrap()
            .get("drill_reliability")
            .is_some()
    );
    let reliability = &a.diagnostics.as_ref().unwrap()["drill_reliability"];
    assert_eq!(reliability["source"], "segment_logprobs");
    assert!((reliability["confidence"].as_f64().unwrap() - (-0.05_f64).exp()).abs() < 1e-9);
    assert_eq!(reliability["noSpeechProbability"], 0.01);
    assert!(
        b.diagnostics
            .as_ref()
            .unwrap()
            .get("drill_reliability")
            .is_none()
    );
    let request = worker.join().unwrap();
    let wire = String::from_utf8_lossy(&request);
    assert!(wire.contains("name=\"prompt\"\r\n\r\nes\nHola"));
    assert!(wire.contains("name=\"language\""));
    {
        let store = app.lock().unwrap();
        assert_eq!(
            store
                .drill_attempts(drill.owner.id(), None, 10)
                .unwrap()
                .attempts
                .len(),
            1
        );
        assert!(
            store
                .conversation_snapshot(chat.owner.id(), None)
                .unwrap()
                .messages
                .is_empty()
        );
        let profile = store.profile().unwrap();
        assert_eq!(profile.global.attempts, 1);
        assert_eq!(
            profile
                .languages
                .iter()
                .find(|l| l.id == "spanish")
                .unwrap()
                .attempts,
            1
        );
        assert_eq!(profile.personas.iter().map(|p| p.attempts).sum::<i32>(), 1);
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
        let receipts = super::super::transcription::views(&store.connection, &drill.owner).unwrap();
        assert!(
            receipts[0]
                .diagnostics
                .as_ref()
                .unwrap()
                .to_string()
                .contains("recognition-fixture")
        );
    }
    assert!(
        transcribe(app.clone(), drill.clone(), input.wav.clone())
            .await
            .is_err()
    );
    drop(app);
    let app = Application::recording_fixture(&dir.path().join("transcription.sqlite3"));
    let mut next = drill.clone();
    next.id = "take-after-restart".into();
    let result = transcribe(app.clone(), next, input.wav.clone())
        .await
        .unwrap();
    assert_eq!(result.diagnostics.unwrap()["cacheHit"], true);
    assert_eq!(
        app.lock()
            .unwrap()
            .drill_attempts(drill.owner.id(), None, 10)
            .unwrap()
            .attempts
            .len(),
        2
    );
    assert_eq!(app.lock().unwrap().profile().unwrap().global.attempts, 1);
    results::set_capacity(&app.lock().unwrap().connection, 0).unwrap();
    assert_eq!(
        results::settings(&app.lock().unwrap().connection)
            .unwrap()
            .result_count,
        0
    );
    assert_eq!(
        app.lock()
            .unwrap()
            .drill_attempts(drill.owner.id(), None, 10)
            .unwrap()
            .attempts
            .len(),
        2
    );
    let saved = crate::speech::recording::results::load(
        &app.lock().unwrap().connection,
        &drill.id,
        &input.wav,
    )
    .unwrap()
    .unwrap();
    assert_eq!(saved.timing.as_ref().unwrap().words[0].word, "Hola");
    assert_eq!(saved.timing.as_ref().unwrap().words[0].start, 0.1);
    assert!(
        crate::speech::recording::results::load(
            &app.lock().unwrap().connection,
            &drill.id,
            b"different"
        )
        .is_err()
    );
    drop(app);
    let app = Application::recording_fixture(&dir.path().join("transcription.sqlite3"));
    assert_eq!(
        crate::speech::recording::results::load(
            &app.lock().unwrap().connection,
            &drill.id,
            &input.wav
        )
        .unwrap()
        .unwrap()
        .timing
        .unwrap()
        .words[0]
            .end,
        end
    );
    let receipt = results::receipt_for_consumer(&app.lock().unwrap().connection, &drill.id)
        .unwrap()
        .unwrap();
    assert_eq!(receipt["state"], "succeeded");
    assert!(!receipt.to_string().contains("Hola"));
    let mut store = app.lock().unwrap();
    let attempt = store
        .drill_attempts(drill.owner.id(), None, 10)
        .unwrap()
        .attempts
        .into_iter()
        .find(|attempt| attempt.transcription_attempt_id.as_deref() == Some(&drill.id))
        .unwrap();
    store.delete_drill_attempt(&attempt.id).unwrap();
    assert!(
        crate::speech::recording::results::result(&store.connection, &drill.id)
            .unwrap()
            .is_none()
    );
    assert!(
        results::receipt_for_consumer(&store.connection, &drill.id)
            .unwrap()
            .is_some()
    );
}

#[tokio::test]
async fn leaving_one_or_all_consumers_does_not_abandon_submitted_recognition() {
    for keep_second in [true, false] {
        let (url, ready, release, worker) = server(reply());
        let (_dir, app, recording, input) = setup(&url);
        // Each consumer now owns an independently retained source recording.
        for id in ["a", "b", "later"] {
            app.lock()
                .unwrap()
                .begin_transcription(id, &recording.owner, &recording.target)
                .unwrap();
        }
        let alive = AtomicBool::new(true);
        let validate = || {
            if alive.load(Ordering::SeqCst) {
                Ok(())
            } else {
                Err(AppError::new(ErrorCode::Conflict, "Recording closed."))
            }
        };
        let second = || if keep_second { Ok(()) } else { validate() };
        let cancel = async {
            ready.await.unwrap();
            alive.store(false, Ordering::SeqCst);
            tokio::time::sleep(Duration::from_millis(120)).await;
            release.send(()).unwrap();
        };
        let (a, b, _) = tokio::join!(
            app.shared_transcription(
                recording.target.clone(),
                input.clone(),
                recording.install.clone(),
                "a",
                validate
            ),
            app.shared_transcription(
                recording.target.clone(),
                input.clone(),
                recording.install.clone(),
                "b",
                second
            ),
            cancel
        );
        assert!(a.is_err());
        assert_eq!(b.is_ok(), keep_second);
        worker.join().unwrap();
        wait_for_settlement(&app, "a", true).await;
        let saved = app
            .shared_transcription(recording.target, input, recording.install, "later", || {
                Ok(())
            })
            .await
            .unwrap();
        // A later consumer may join the settled producer before its delivery
        // task exits, or adopt its cache afterwards. Both must reuse exactly the
        // same native execution, without another provider submission.
        let store = app.lock().unwrap();
        let first = results::receipt_for_consumer(&store.connection, "a")
            .unwrap()
            .unwrap();
        let later = results::receipt_for_consumer(&store.connection, "later")
            .unwrap()
            .unwrap();
        assert_eq!(first["nativeEngine"], later["nativeEngine"]);
        assert_eq!(first["nativeExecution"], later["nativeExecution"]);
        assert!(first["nativeExecution"].is_u64());
        drop(store);
        let decoded: TranscriptionResult = serde_json::from_slice(&saved.payload).unwrap();
        assert_eq!(decoded.timing.unwrap().words[0].word, "Hola");
        assert_eq!(app.lock().unwrap().profile().unwrap().global.attempts, 1);
    }
}

async fn wait_for_settlement(app: &Application, consumer: &str, require_adoption: bool) {
    let mut last_state = String::new();
    tokio::time::timeout(FIXTURE_TIMEOUT, async {
        loop {
            let receipt = results::receipt_for_consumer(&app.lock().unwrap().connection, consumer)
                .unwrap()
                .unwrap();
            last_state = format!("receipt state: {}", receipt["state"]);
            if receipt["state"] == "succeeded" {
                if !require_adoption {
                    break;
                }
                let store = app.lock().unwrap();
                let history = crate::ai::workspace_graph::inspection(
                    &store.connection,
                    receipt["nativeEngine"].as_str().unwrap(),
                )
                .unwrap();
                let view = history
                    .snapshot(
                        receipt["nativeRun"].as_str().unwrap(),
                        crate::ai::graph::ExportLimits {
                            bytes: 4 * 1024 * 1024,
                            attempts: 4096,
                        },
                    )
                    .unwrap();
                let states: Vec<_> = view.nodes.values().collect();
                last_state.push_str(&format!(", node states: {states:?}"));
                assert!(
                    !states.contains(&&crate::ai::graph::Disposition::Cancelled),
                    "Closing consumers must not cancel already submitted recognition"
                );
                if states
                    .iter()
                    .all(|state| **state == crate::ai::graph::Disposition::Adopted)
                {
                    break;
                }
            }
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
    })
    .await
    .unwrap_or_else(|_| panic!("Recognition did not settle for {consumer}: {last_state}"));
}

#[tokio::test]
async fn abandoned_recognition_can_settle_after_a_slow_provider_response() {
    let (url, ready, release, worker) = server(reply());
    let (_dir, app, recording, input) = setup(&url);
    app.lock()
        .unwrap()
        .begin_transcription("slow", &recording.owner, &recording.target)
        .unwrap();
    let alive = AtomicBool::new(true);
    let cancel = async {
        ready.await.unwrap();
        alive.store(false, Ordering::SeqCst);
    };
    let (result, _) = tokio::join!(
        app.shared_transcription(recording.target, input, recording.install, "slow", || {
            if alive.load(Ordering::SeqCst) {
                Ok(())
            } else {
                Err(AppError::new(ErrorCode::Conflict, "Recording closed."))
            }
        }),
        cancel
    );
    assert!(result.is_err());
    // Start the actual settlement wait before delivering the provider response.
    // This is a behavioral wait, not a three-second performance requirement.
    let respond = async {
        tokio::time::sleep(Duration::from_millis(3100)).await;
        release.send(()).unwrap();
    };
    tokio::join!(wait_for_settlement(&app, "slow", true), respond);
    worker.join().unwrap();
    assert_eq!(app.lock().unwrap().profile().unwrap().global.attempts, 1);
}

#[tokio::test]
async fn failed_provider_contract_keeps_metadata_without_publishing_or_caching() {
    let (url, ready, release, worker) = server(json!({"request_id":"failed-recognition","text":3}));
    let (_dir, app, recording, input) = setup(&url);
    let gate = async {
        ready.await.unwrap();
        release.send(()).unwrap();
    };
    let (result, _) = tokio::join!(transcribe(app.clone(), recording.clone(), input.wav), gate);
    assert!(result.is_err());
    worker.join().unwrap();
    let store = app.lock().unwrap();
    let receipt = results::receipt_for_consumer(&store.connection, &recording.id)
        .unwrap()
        .unwrap();
    assert_eq!(receipt["state"], "unknown");
    assert_eq!(receipt["dispatched"], true);
    assert!(receipt.to_string().contains("failed-recognition"));
    assert_eq!(
        results::settings(&store.connection).unwrap().result_count,
        0
    );
    assert!(
        store
            .drill_attempts(recording.owner.id(), None, 10)
            .unwrap()
            .attempts
            .is_empty()
    );
    assert_eq!(store.profile().unwrap().global.attempts, 1);
}

#[test]
fn identity_includes_audio_context_language_and_access_but_not_recording_owner() {
    let (_dir, _app, recording, input) = setup("http://127.0.0.1:1/v1");
    let key = |target: &access::ResolvedTarget, input: &TranscriptionRequest, install: &str| {
        results::transcription::request_key(target, input, install).unwrap()
    };
    let original = key(&recording.target, &input, &recording.install);
    let mut changed = input.clone();
    changed.wav[50] ^= 1;
    assert_ne!(
        original,
        key(&recording.target, &changed, &recording.install)
    );
    let mut changed = input.clone();
    changed.context = Some("Different prompt".into());
    assert_ne!(
        original,
        key(&recording.target, &changed, &recording.install)
    );
    let mut changed = input.clone();
    changed.language.variety_id.push_str("-different");
    assert_ne!(
        original,
        key(&recording.target, &changed, &recording.install)
    );
    let mut target = recording.target.clone();
    target.model.push_str("-different");
    assert_ne!(original, key(&target, &input, &recording.install));
    assert_ne!(
        original,
        key(&recording.target, &input, "another-workspace")
    );
}

#[tokio::test]
async fn removing_the_recording_owner_prevents_publication_but_retains_paid_execution() {
    let (url, ready, release, worker) = server(reply());
    let (_dir, app, recording, input) = setup(&url);
    let close = async {
        ready.await.unwrap();
        app.lock()
            .unwrap()
            .connection
            .execute(
                "UPDATE drill_items SET archived=1 WHERE id=?1",
                [recording.owner.id()],
            )
            .unwrap();
        tokio::time::sleep(Duration::from_millis(120)).await;
        release.send(()).unwrap();
    };
    let (result, _) = tokio::join!(transcribe(app.clone(), recording.clone(), input.wav), close);
    assert!(result.is_err());
    worker.join().unwrap();
    wait_for_settlement(&app, &recording.id, false).await;
    let store = app.lock().unwrap();
    let receipts = super::super::transcription::views(&store.connection, &recording.owner).unwrap();
    assert_eq!(receipts[0].state, "unknown");
    assert_eq!(
        receipts[0].diagnostics.as_ref().unwrap()["sourceExecution"]["state"],
        "succeeded"
    );
    let attempts: i64 = store
        .connection
        .query_row("SELECT count(*) FROM drill_attempts", [], |r| r.get(0))
        .unwrap();
    assert_eq!(attempts, 0);
    assert_eq!(store.profile().unwrap().global.attempts, 1);
}

#[tokio::test]
async fn cache_write_failure_retains_provider_metadata_and_admission_failure_is_not_paid() {
    let (url, ready, release, worker) = server(reply());
    let (_dir, app, recording, input) = setup(&url);
    app.lock().unwrap().connection.execute_batch("CREATE TRIGGER reject_cache BEFORE INSERT ON workspace_transcription_cache BEGIN SELECT RAISE(ABORT,'cache refused'); END;").unwrap();
    let gate = async {
        ready.await.unwrap();
        release.send(()).unwrap();
    };
    let (result, _) = tokio::join!(
        transcribe(app.clone(), recording.clone(), input.wav.clone()),
        gate
    );
    assert!(result.is_err());
    worker.join().unwrap();
    let receipt = results::receipt_for_consumer(&app.lock().unwrap().connection, &recording.id)
        .unwrap()
        .unwrap();
    assert_eq!(receipt["state"], "failed");
    assert!(receipt.to_string().contains("recognition-fixture"));
    assert!(!receipt.to_string().contains("Hola"));
    app.lock()
        .unwrap()
        .begin_transcription("held", &recording.owner, &recording.target)
        .unwrap();
    app.lock()
        .unwrap()
        .connection
        .execute("UPDATE ai_config SET paused=1", [])
        .unwrap();
    let held = app
        .shared_transcription(
            recording.target,
            input,
            recording.install,
            "held",
            || Ok(()),
        )
        .await;
    assert!(held.is_err());
    let receipt = results::receipt_for_consumer(&app.lock().unwrap().connection, "held")
        .unwrap()
        .unwrap();
    assert_eq!(receipt["dispatched"], false);
    assert_eq!(app.lock().unwrap().profile().unwrap().global.attempts, 1);
}

fn carries(request: &[u8], wav: &[u8]) -> bool {
    request.windows(wav.len()).any(|window| window == wav)
}

#[tokio::test]
async fn a_failed_take_is_held_and_retried_with_its_audio_as_a_new_attempt() {
    failed_take_retry(false).await;
}

#[tokio::test]
async fn conversation_failed_take_retries_through_native_execution() {
    failed_take_retry(true).await;
}

async fn failed_take_retry(conversation: bool) {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let (url, ready, release, worker) = server_on(
        listener.try_clone().unwrap(),
        json!({"request_id":"failed-recognition","text":3}),
    );
    let (_dir, app, mut recording, input) = setup(&url);
    if conversation {
        let mut store = app.lock().unwrap();
        store.prepare_chat().unwrap();
        recording.owner =
            RecordingOwner::Conversation(store.snapshot().unwrap().conversations[0].id.clone());
        recording.visit = None;
    }
    let gate = async {
        ready.await.unwrap();
        release.send(()).unwrap();
    };
    let (result, _) = tokio::join!(
        transcribe_holding_failure(app.clone(), recording.clone(), input.wav.clone()),
        gate
    );
    assert!(result.is_err());
    assert!(carries(&worker.join().unwrap(), &input.wav));
    assert!(app.failed_take.lock().unwrap().held.is_some());

    let (_, ready, release, worker) = server_on(listener, reply());
    let gate = async {
        ready.await.unwrap();
        release.send(()).unwrap();
    };
    let (retried, _) = tokio::join!(retry_held_take(app.clone(), &recording.id), gate);
    let retried = retried.unwrap();
    // The same audio, as a new attempt with its own identity.
    assert!(carries(&worker.join().unwrap(), &input.wav));
    assert_eq!(retried.text, "Hola");
    assert_ne!(retried.inspection.recording_id, recording.id);
    assert!(app.failed_take.lock().unwrap().held.is_none());
    // The failed attempt stays recorded for AI activity.
    let store = app.lock().unwrap();
    let failed = results::receipt_for_consumer(&store.connection, &recording.id)
        .unwrap()
        .unwrap();
    assert_eq!(failed["state"], "unknown");
    assert!(failed.to_string().contains("failed-recognition"));
}

#[tokio::test]
async fn retrying_a_take_that_is_not_held_fails_without_provider_work() {
    let (_dir, app, _recording, _input) = setup("http://127.0.0.1:9/v1");
    let error = retry_held_take(app.clone(), "not-held").await.unwrap_err();
    assert!(matches!(error.code, ErrorCode::NotFound));
}

#[tokio::test]
async fn late_transcription_and_retry_cannot_replace_a_newer_failed_take() {
    for (retry, succeeds) in [(false, false), (false, true), (true, false)] {
        let body = if succeeds { reply() } else { json!({"text":3}) };
        let (url, ready, release, worker) = server(body);
        let (_dir, app, recording, input) = setup(&url);
        if retry {
            app.failed_take.lock().unwrap().held = Some(FailedTake {
                take: recording.id.clone(),
                request: recording.clone(),
                wav: input.wav.clone(),
            });
        }
        let gate = async {
            ready.await.unwrap();
            let mut newer = recording.clone();
            newer.id = "newer-take".into();
            let mut slot = app.failed_take.lock().unwrap();
            slot.start(&newer.id);
            slot.held = Some(FailedTake {
                take: newer.id.clone(),
                request: newer,
                wav: input.wav.clone(),
            });
            release.send(()).unwrap();
        };
        let run = async {
            if retry {
                retry_held_take(app.clone(), &recording.id).await
            } else {
                transcribe_holding_failure(app.clone(), recording.clone(), input.wav.clone()).await
            }
        };
        let (result, _) = tokio::join!(run, gate);
        assert_eq!(result.is_ok(), succeeds);
        worker.join().unwrap();
        assert_eq!(
            app.failed_take.lock().unwrap().held.as_ref().unwrap().take,
            "newer-take"
        );
    }
}

#[tokio::test]
async fn retry_refuses_a_changed_destination_and_retains_the_audio() {
    let (_dir, app, recording, input) = setup("http://127.0.0.1:9/v1");
    app.failed_take.lock().unwrap().held = Some(FailedTake {
        take: recording.id.clone(),
        request: recording.clone(),
        wav: input.wav,
    });
    app.lock().unwrap().connection.execute(
        "UPDATE ai_config SET custom_config=json_set(custom_config,'$.baseUrl','http://127.0.0.1:8/v1')", []
    ).unwrap();
    let error = retry_held_take(app.clone(), &recording.id)
        .await
        .unwrap_err();
    assert_eq!(error.code, ErrorCode::Conflict);
    assert!(app.failed_take.lock().unwrap().held.is_some());
    assert_eq!(app.lock().unwrap().profile().unwrap().global.attempts, 0);
}

#[tokio::test]
async fn a_retry_that_fails_again_keeps_the_take_held() {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let (url, ready, release, worker) = server_on(
        listener.try_clone().unwrap(),
        json!({"request_id":"first-failure","text":3}),
    );
    let (_dir, app, recording, input) = setup(&url);
    let gate = async {
        ready.await.unwrap();
        release.send(()).unwrap();
    };
    let (first, _) = tokio::join!(
        transcribe_holding_failure(app.clone(), recording.clone(), input.wav.clone()),
        gate
    );
    assert!(first.is_err());
    worker.join().unwrap();
    let (_, ready, release, worker) =
        server_on(listener, json!({"request_id":"second-failure","text":3}));
    let gate = async {
        ready.await.unwrap();
        release.send(()).unwrap();
    };
    let (again, _) = tokio::join!(retry_held_take(app.clone(), &recording.id), gate);
    assert!(again.is_err());
    worker.join().unwrap();
    assert!(app.failed_take.lock().unwrap().held.is_some());
}
