use std::{
    io::{Read, Write},
    time::Duration,
};
/// Serve one grouped structured request on the custom route and return the
/// submitted payload. `content` builds the reply from the source message.
pub(crate) fn structured_server(
    content: impl FnOnce(&str) -> String + Send + 'static,
) -> (String, std::thread::JoinHandle<serde_json::Value>) {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let base = format!("http://{}/v1", listener.local_addr().unwrap());
    let worker = std::thread::spawn(move || serve_one(&listener, content));
    (base, worker)
}

pub(crate) fn structured_sequence(
    count: usize,
    mut content: impl FnMut(&str) -> String + Send + 'static,
) -> (String, std::thread::JoinHandle<Vec<serde_json::Value>>) {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let base = format!("http://{}/v1", listener.local_addr().unwrap());
    let worker = std::thread::spawn(move || {
        (0..count)
            .map(|_| serve_one(&listener, &mut content))
            .collect()
    });
    (base, worker)
}

fn serve_one(
    listener: &std::net::TcpListener,
    content: impl FnOnce(&str) -> String,
) -> serde_json::Value {
    let (mut socket, _) = listener.accept().unwrap();
    socket
        .set_read_timeout(Some(Duration::from_secs(5)))
        .unwrap();
    let mut received = Vec::new();
    let (start, length) = loop {
        let mut buffer = [0; 4096];
        let count = socket.read(&mut buffer).unwrap();
        assert!(count > 0);
        received.extend_from_slice(&buffer[..count]);
        if let Some(start) = received.windows(4).position(|w| w == b"\r\n\r\n") {
            let headers = String::from_utf8_lossy(&received[..start]).to_string();
            if headers.starts_with("GET /v1/protocol ") {
                let body = r#"{"operations_versions":[3]}"#;
                write!(socket, "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}", body.len()).unwrap();
                return serve_one(listener, content);
            }
            assert!(headers.starts_with("POST /v1/operations "));
            let length: usize = headers
                .lines()
                .find_map(|line| {
                    line.to_lowercase()
                        .strip_prefix("content-length: ")
                        .and_then(|n| n.parse().ok())
                })
                .unwrap();
            if received.len() >= start + 4 + length {
                break (start + 4, length);
            }
        }
    };
    let payload: serde_json::Value =
        serde_json::from_slice(&received[start..start + length]).unwrap();
    let item = &payload["items"][0];
    let messages = item["request"]["messages"].as_array().unwrap();
    let content = content(messages.last().unwrap()["content"].as_str().unwrap());
    let mut body = serde_json::to_vec(&serde_json::json!({"type":"result","operation_id":item["operation_id"],"attempt_id":item["attempt_id"],
        "response":{"id":"structured-receipt","model":"actual-fast","choices":[{"finish_reason":"stop","message":{"content":content}}],"usage":{"prompt_tokens":21,"completion_tokens":4}}})).unwrap();
    body.push(b'\n');
    body.extend(serde_json::to_vec(&serde_json::json!({"type":"complete","count":1})).unwrap());
    body.push(b'\n');
    write!(socket, "HTTP/1.1 200 OK\r\nContent-Type: application/x-ndjson\r\nContent-Length: {}\r\nConnection: close\r\n\r\n", body.len()).unwrap();
    socket.write_all(&body).unwrap();
    payload
}

/// A translation reply that echoes the source into the translation contract.
pub(crate) fn translation_server(
    translation: serde_json::Value,
) -> (String, std::thread::JoinHandle<serde_json::Value>) {
    structured_server(move |source| {
        serde_json::json!({"source":source,"translation":translation}).to_string()
    })
}
