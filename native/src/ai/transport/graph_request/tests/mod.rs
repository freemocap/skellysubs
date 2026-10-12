use super::*;
use crate::ai::{connections::access::ResolvedTarget, graph::*};
use crate::conversations::execution::coach_graph;
use crate::model::ConnectionRoute;
use serde_json::json;
use std::{
    collections::BTreeMap,
    io::{Read, Write},
    sync::Arc,
    time::Duration,
};

fn server(mode: &'static str) -> (String, std::thread::JoinHandle<serde_json::Value>) {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let url = format!("http://{}/v1/operations", listener.local_addr().unwrap());
    let worker = std::thread::spawn(move || {
        let (mut socket, _) = listener.accept().unwrap();
        socket
            .set_read_timeout(Some(Duration::from_secs(5)))
            .unwrap();
        let mut bytes = Vec::new();
        let payload = loop {
            let mut buffer = [0; 4096];
            let read = socket.read(&mut buffer).unwrap();
            assert!(read > 0);
            bytes.extend_from_slice(&buffer[..read]);
            if let Some(end) = bytes.windows(4).position(|v| v == b"\r\n\r\n") {
                let headers = String::from_utf8_lossy(&bytes[..end]);
                assert!(headers.starts_with("POST /v1/operations "));
                let len: usize = headers
                    .lines()
                    .find_map(|line| {
                        line.to_lowercase()
                            .strip_prefix("content-length: ")
                            .and_then(|v| v.parse().ok())
                    })
                    .unwrap();
                if bytes.len() >= end + 4 + len {
                    break serde_json::from_slice::<serde_json::Value>(
                        &bytes[end + 4..end + 4 + len],
                    )
                    .unwrap();
                }
            }
        };
        let item = &payload["items"][0];
        let text = if mode == "structured" {
            r#"{"answer":"Private reply"}"#
        } else {
            "Private reply"
        };
        let mut events = vec![
            json!({"type":"frames","operation_id":item["operation_id"],"attempt_id":item["attempt_id"],"offset":0,
            "frames":[{"id":"provider-17","model":"actual","choices":[{"index":0,"delta":{"content":text}}]}]}),
        ];
        if mode == "structured" {
            events.clear();
        }
        if mode != "interrupted" {
            let response = if mode == "malformed" {
                json!({"id":"provider-17","model":"actual","usage":{"prompt_tokens":7},"choices":null})
            } else {
                json!({"id":"provider-17","model":"actual","usage":{"prompt_tokens":7,"completion_tokens":3,"cost":0.0005},
                    "choices":[{"finish_reason":"stop","message":{"content":text}}]})
            };
            events.push(json!({"type":"result","operation_id":item["operation_id"],"attempt_id":item["attempt_id"],"response":response}));
            if mode != "missing_end" {
                events.push(json!({"type":"complete","count":1}));
            }
        }
        let body = events.iter().map(|e| format!("{e}\n")).collect::<String>();
        write!(socket,"HTTP/1.1 200 OK\r\nContent-Type: application/x-ndjson\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",body.len()).unwrap();
        payload
    });
    (url, worker)
}

async fn run(mode: &'static str, bytes: usize) -> (InvocationReport, Outcome, serde_json::Value) {
    let (url, worker) = server(mode);
    let retained = Arc::new(Mutex::new(None));
    let captured = retained.clone();
    let graph = Arc::new(
        coach_graph::compile(Arc::new(move |context, request| {
            let captured = captured.clone();
            Box::pin(async move {
                let request =
                    request.text_request("durable-attempt".into(), "durable-operation".into());
                let schema = json!({"type":"object","properties":{"answer":{"type":"string"}},"required":["answer"],"additionalProperties":false});
                let output = if mode == "structured" {
                    provider::RequestOutput::JsonSchema { max_output_tokens:512,name:"graph_test",schema:&schema }
                } else { provider::RequestOutput::Prose };
                let outcome = execute(&provider::client().unwrap(), "", &request, output, &context, true).await;
                let result = match &outcome.item {
                    Some(Ok(completion)) => Ok(completion.text.clone()),
                    _ => Err(Fault {
                        code: "transport_failed".into(),
                        path: "response".into(),
                    }),
                };
                *captured.lock().unwrap() = Some(outcome);
                result
            })
        }))
        .unwrap(),
    );
    let inputs = coach_graph::capture(
        vec![
            provider::PromptMessage {
                role: "system".into(),
                content: "Private instructions".into(),
            },
            provider::PromptMessage {
                role: "user".into(),
                content: "Private question".into(),
            },
        ],
        vec![],
        &ResolvedTarget {
            audio_resolution: None,
            route: ConnectionRoute::Custom,
            revision: 1,
            url,
            model: "requested".into(),
            credential: None,
        },
        "install",
    )
    .unwrap();
    let mut engine = Engine::new([graph.clone()]).unwrap();
    engine
        .apply(Event::Begin {
            run: "run".into(),
            artifact: graph.identity().into(),
            inputs,
            scope: "scope".into(),
            policy: BTreeMap::new(),
        })
        .unwrap();
    let work = engine
        .apply(Event::Advance(Capacity {
            local: 1,
            provider: 0,
        }))
        .unwrap()
        .remove(0);
    let local = engine
        .claim(work.execution)
        .unwrap()
        .execute(EvidenceLimits {
            observations: 8,
            bytes: 100_000,
        })
        .await;
    engine.apply(Event::SettleObserved(local)).unwrap();
    let attempt = engine.inspect("run").unwrap().attempts[coach_graph::CONTEXT][0].id;
    engine
        .apply(Event::Adopt {
            run: "run".into(),
            node: coach_graph::CONTEXT.into(),
            attempt,
        })
        .unwrap();
    let work = engine
        .apply(Event::Advance(Capacity {
            local: 1,
            provider: 1,
        }))
        .unwrap()
        .remove(0);
    let report = engine
        .claim(work.execution)
        .unwrap()
        .execute_with_provisional(
            EvidenceLimits {
                observations: 8,
                bytes,
            },
            ProvisionalLimits { bytes: 4096 },
        )
        .await;
    let outcome = retained.lock().unwrap().take().unwrap();
    (report, outcome, worker.join().unwrap())
}

#[tokio::test]
async fn structured_native_transport_keeps_the_explicit_schema_and_response_evidence() {
    let (report, outcome, payload) = run("structured", 100_000).await;
    assert!(outcome.transport.is_ok());
    assert!(outcome.capture_failure.is_none());
    assert_eq!(
        report.outcome.unwrap()["text"],
        r#"{"answer":"Private reply"}"#
    );
    let item = &payload["items"][0];
    assert!(item.get("deltas").is_none());
    assert_eq!(
        item["request"]["response_format"]["json_schema"]["name"],
        "graph_test"
    );
    assert_eq!(item["request"]["max_tokens"], 512);
    assert_eq!(
        item["request"]["response_format"]["json_schema"]["schema"]["required"],
        json!(["answer"])
    );
    assert_eq!(
        report.observations[0].actual_model.as_deref(),
        Some("actual")
    );
    assert!(
        !serde_json::to_string(&report.observations)
            .unwrap()
            .contains("Private")
    );
}

#[tokio::test]
async fn native_coach_uses_real_stream_transport_and_preserves_wire_identity() {
    let (report, outcome, payload) = run("success", 100_000).await;
    assert!(outcome.transport.is_ok());
    assert!(outcome.capture_failure.is_none());
    assert_eq!(report.outcome.unwrap()["text"], "Private reply");
    assert_eq!(report.provisional.unwrap().text, "Private reply");
    assert_eq!(payload["items"][0]["attempt_id"], "durable-attempt");
    assert_eq!(payload["items"][0]["operation_id"], "durableoperation");
    assert_eq!(
        report.observations[0].actual_model.as_deref(),
        Some("actual")
    );
    assert!(
        !serde_json::to_string(&report.observations)
            .unwrap()
            .contains("Private")
    );
}

#[tokio::test]
async fn received_item_survives_missing_end_and_interruption_keeps_provisional_evidence() {
    for mode in ["missing_end", "interrupted", "malformed"] {
        let (report, outcome, _) = run(mode, 100_000).await;
        assert_eq!(outcome.transport.is_ok(), mode == "malformed");
        assert_eq!(report.outcome.is_ok(), mode == "missing_end");
        assert_eq!(report.provisional.unwrap().text, "Private reply");
        assert!(!report.observations.is_empty());
        let metadata = serde_json::to_string(&report.observations).unwrap();
        assert!(!metadata.contains("Private"));
        if mode == "malformed" {
            assert!(metadata.contains("provider-17"));
        }
        if mode == "missing_end" {
            assert_eq!(report.observations.len(), 2);
        }
    }
}

#[tokio::test]
async fn capture_failure_cannot_turn_into_success_or_erase_original_completion() {
    let (report, outcome, _) = run("success", 0).await;
    assert!(outcome.item.unwrap().is_ok());
    assert!(outcome.capture_failure.is_some());
    assert_eq!(report.outcome.unwrap_err().code, "evidence_limit");
    assert!(report.evidence_failure.is_some());
}
