//! Bounded NDJSON decoding. Valid item outcomes can be persisted before EOF.
use crate::ai::transport::provider;
use crate::ai::transport::provider::Completion;
use crate::model::AppError;
use crate::model::ErrorCode;
use crate::model::Result;
use serde::Deserialize;
use std::collections::HashMap;

const RESPONSE_TEXT_LIMIT: usize = 262_144;
const LINE_LIMIT: usize = 4 * 1024 * 1024 + 4096;
pub(crate) const MAX_ITEMS: usize = 8;
const STREAM_LIMIT: usize = MAX_ITEMS * LINE_LIMIT + 1024;
/// Additional allowance for bounded frame envelopes alongside terminal receipts.
const DELTA_ALLOWANCE: usize = RESPONSE_TEXT_LIMIT * 6 + 20 * 180 * 256;

fn unknown() -> AppError {
    AppError::new(
        ErrorCode::UnknownOutcome,
        "Grouped response is incomplete or invalid. Unconfirmed operations may have incurred charges; no automatic retry was made.",
    )
}

fn network(error: &reqwest::Error, stage: &str) -> AppError {
    let mut failure = crate::diagnostics::response::network(error, stage);
    if error.is_connect() || stage == "grouped_protocol" {
        failure.code = ErrorCode::Provider;
        failure.message = "Could not reach the AI server. No inference request was accepted. Check the connection and retry.".into();
    }
    failure
}

fn protocol_timeout() -> AppError {
    AppError::new(ErrorCode::Provider, "AI server connection check timed out. No inference request was sent. Retry when the server is available.")
        .with_diagnostics(serde_json::json!({"stage":"grouped_protocol","reason":"timeout"}))
}

#[derive(Deserialize)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
enum Event {
    Result {
        operation_id: String,
        attempt_id: String,
        response: serde_json::Value,
    },
    Duplicate {
        operation_id: String,
        attempt_id: String,
        state: String,
    },
    Error {
        operation_id: String,
        attempt_id: String,
        code: String,
        status: u16,
        retry_after: Option<u32>,
        #[serde(default)]
        diagnostics: Option<serde_json::Value>,
        #[serde(default)]
        request_id: Option<String>,
    },
    Complete {
        count: usize,
    },
    /// Original provider frames; `offset` counts previously received frames.
    Frames {
        operation_id: String,
        attempt_id: String,
        offset: usize,
        frames: Vec<serde_json::Value>,
    },
}

pub struct Decoder {
    pending: HashMap<String, String>,
    count: usize,
    line: Vec<u8>,
    bytes: usize,
    limit: usize,
    complete: bool,
    /// Native frame reconstruction; absent for groups that did not request streaming.
    deltas: Option<HashMap<String, provider::stream::Stream>>,
}
impl Decoder {
    pub fn new(identities: impl IntoIterator<Item = (String, String)>) -> Result<Self> {
        let mut pending = HashMap::new();
        for (operation, attempt) in identities {
            if pending.values().any(|existing| existing == &attempt)
                || pending.insert(operation, attempt).is_some()
            {
                return Err(unknown());
            }
        }
        let count = pending.len();
        if !(1..=MAX_ITEMS).contains(&count) {
            return Err(unknown());
        }
        Ok(Self {
            pending,
            count,
            line: Vec::new(),
            bytes: 0,
            limit: STREAM_LIMIT,
            complete: false,
            deltas: None,
        })
    }
    /// A frame decoder for pending streamed items.
    pub fn with_deltas(identities: impl IntoIterator<Item = (String, String)>) -> Result<Self> {
        let mut decoder = Self::new(identities)?;
        decoder.limit = STREAM_LIMIT + decoder.count * DELTA_ALLOWANCE;
        decoder.deltas = Some(HashMap::new());
        Ok(decoder)
    }
    pub fn push(
        &mut self,
        chunk: &[u8],
        publish: impl FnMut(&str, Result<Completion>) -> Result<()>,
    ) -> Result<()> {
        self.push_with_deltas(chunk, publish, |_, _| Ok(()))
    }
    /// Feed bytes. `on_delta` receives an item's full text so far after each
    /// accepted delta; a delta never publishes and never ends an item.
    pub fn push_with_deltas(
        &mut self,
        chunk: &[u8],
        mut publish: impl FnMut(&str, Result<Completion>) -> Result<()>,
        mut on_delta: impl FnMut(&str, &str) -> Result<()>,
    ) -> Result<()> {
        self.bytes = self.bytes.checked_add(chunk.len()).ok_or_else(unknown)?;
        if self.bytes > self.limit {
            return Err(unknown());
        }
        for byte in chunk {
            if self.complete {
                return Err(unknown());
            }
            if *byte != b'\n' {
                if self.line.len() >= LINE_LIMIT {
                    return Err(unknown());
                }
                self.line.push(*byte);
                continue;
            }
            let event: Event = serde_json::from_slice(&self.line).map_err(|cause| {
                crate::diagnostics::response::json_context(&cause, "grouped_decode", unknown())
            })?;
            self.line.clear();
            let (operation, attempt, result) = match event {
                Event::Frames {
                    operation_id,
                    attempt_id,
                    offset,
                    frames,
                } => {
                    let texts = self.deltas.as_mut().ok_or_else(unknown)?;
                    // Only for an item still pending: never after its result.
                    if self.pending.get(&operation_id) != Some(&attempt_id) {
                        return Err(unknown());
                    }
                    let stream = texts.entry(operation_id.clone()).or_default();
                    if offset != stream.frames.len() {
                        return Err(unknown());
                    }
                    for frame in frames {
                        stream.push(frame).map_err(|_| unknown())?;
                    }
                    on_delta(&operation_id, &stream.text)?;
                    continue;
                }
                Event::Complete { count } => {
                    if count != self.count || !self.pending.is_empty() {
                        return Err(unknown());
                    }
                    self.complete = true;
                    continue;
                }
                Event::Result {
                    operation_id,
                    attempt_id,
                    response,
                } => (
                    operation_id.clone(),
                    attempt_id,
                    if response["stream_complete"] == true {
                        self.deltas
                            .as_ref()
                            .and_then(|streams| streams.get(&operation_id))
                            .ok_or_else(unknown)?
                            .completion(&response)
                    } else {
                        provider::decode(&serde_json::to_vec(&response)?)
                    },
                ),
                Event::Duplicate {
                    operation_id,
                    attempt_id,
                    state,
                } => {
                    if !matches!(
                        state.as_str(),
                        "running" | "succeeded" | "failed" | "unknown"
                    ) {
                        return Err(unknown());
                    }
                    (
                        operation_id,
                        attempt_id,
                        Err(AppError::new(
                            ErrorCode::UnknownOutcome,
                            "The server has already admitted this attempt. Its result is not available for replay; no additional request was started.",
                        )),
                    )
                }
                Event::Error {
                    operation_id,
                    attempt_id,
                    code,
                    status,
                    retry_after,
                    diagnostics,
                    request_id,
                } => {
                    if !(400..=599).contains(&status)
                        || code.len() > 64
                        || !code
                            .bytes()
                            .all(|c| c.is_ascii_uppercase() || c.is_ascii_digit() || c == b'_')
                    {
                        return Err(unknown());
                    }
                    let provider_message = (status == 502)
                        .then(|| crate::ai::hosted::provider_failure_message(&code))
                        .flatten();
                    let mut error = if let Some(message) = provider_message {
                        AppError::new(ErrorCode::UnknownOutcome, message)
                    } else if status == 429 {
                        crate::ai::hosted::limit_error(
                            &serde_json::to_vec(
                                &serde_json::json!({"code":code,"request_id":request_id}),
                            )?,
                            retry_after,
                        )
                    } else if status >= 500 {
                        AppError::new(
                            ErrorCode::UnknownOutcome,
                            format!(
                                "The server reported HTTP {status} for this operation. Usage is unconfirmed; no automatic retry was made."
                            ),
                        )
                    } else {
                        AppError::new(
                            ErrorCode::Provider,
                            format!(
                                "Server rejected this operation (HTTP {status}). No automatic retry was made."
                            ),
                        )
                    };
                    if let Some(details) = diagnostics {
                        let details = crate::diagnostics::response::metadata(&details, &[]);
                        if let Some(reason) = crate::diagnostics::response::reason(&details) {
                            error
                                .message
                                .push_str(&format!(" Provider reason: {reason}"));
                        }
                        error.diagnostics = Some(
                            serde_json::json!({"stage":"grouped_operation", "response":details, "request_id":request_id, "status":status, "code":code}),
                        );
                    }
                    if error.diagnostics.is_none() {
                        error.diagnostics = Some(
                            serde_json::json!({"stage":"grouped_operation", "request_id":request_id, "status":status, "code":code}),
                        );
                    }
                    if let Some(history) = error
                        .diagnostics
                        .as_ref()
                        .and_then(|d| d.pointer("/response/automatic_retries"))
                        .and_then(serde_json::Value::as_array)
                        .cloned()
                    {
                        crate::ai::policy::retry::annotate(&mut error, &history);
                    }
                    (operation_id, attempt_id, Err(error))
                }
            };
            if self.pending.get(&operation) != Some(&attempt) {
                return Err(unknown());
            }
            publish(&operation, result)?;
            self.pending.remove(&operation);
        }
        Ok(())
    }
    pub fn finish(self) -> Result<()> {
        if self.complete && self.line.is_empty() && self.pending.is_empty() {
            Ok(())
        } else {
            Err(unknown())
        }
    }
}

pub async fn request(
    client: &reqwest::Client,
    key: &str,
    dispatches: &[crate::ai::transport::text_request::TextRequest],
    publish: impl FnMut(usize, Result<Completion>) -> Result<()>,
) -> Result<()> {
    let outputs = vec![provider::RequestOutput::Prose; dispatches.len()];
    request_with_outputs(client, key, dispatches, &outputs, publish).await
}

/// One explicit output contract per item; mismatches fail before HTTP submission.
pub async fn request_with_outputs(
    client: &reqwest::Client,
    key: &str,
    dispatches: &[crate::ai::transport::text_request::TextRequest],
    outputs: &[provider::RequestOutput<'_>],
    publish: impl FnMut(usize, Result<Completion>) -> Result<()>,
) -> Result<()> {
    request_streaming(client, key, dispatches, outputs, false, publish, |_, _| {}).await
}

/// Like `request_with_outputs`; with `deltas`, requests version 3 provider frames
/// and assembles prose locally as they arrive. `on_delta` receives an item's
/// full text so far. Only call with `deltas` for a server that advertises it.
pub async fn request_streaming(
    client: &reqwest::Client,
    key: &str,
    dispatches: &[crate::ai::transport::text_request::TextRequest],
    outputs: &[provider::RequestOutput<'_>],
    deltas: bool,
    publish: impl FnMut(usize, Result<Completion>) -> Result<()>,
    on_delta: impl FnMut(usize, &str),
) -> Result<()> {
    #[cfg(feature = "workflow-capture")]
    {
        super::workflow::text::request(client, key, dispatches, outputs, deltas, publish, on_delta)
            .await
    }
    #[cfg(not(feature = "workflow-capture"))]
    {
        request_streaming_live(client, key, dispatches, outputs, deltas, publish, on_delta).await
    }
}

pub(super) async fn request_streaming_live(
    client: &reqwest::Client,
    key: &str,
    dispatches: &[crate::ai::transport::text_request::TextRequest],
    outputs: &[provider::RequestOutput<'_>],
    deltas: bool,
    mut publish: impl FnMut(usize, Result<Completion>) -> Result<()>,
    mut on_delta: impl FnMut(usize, &str),
) -> Result<()> {
    #[cfg(feature = "workflow-capture")]
    crate::ai::transport::workflow::require_live_network()?;
    if dispatches.len() != outputs.len() {
        return Err(AppError::new(
            ErrorCode::Validation,
            "Each grouped operation requires one output contract.",
        ));
    }
    let first = dispatches.first().ok_or_else(unknown)?;
    if dispatches.len() > MAX_ITEMS || dispatches.iter().any(|d| !compatible(first, d)) {
        return Err(unknown());
    }
    let mut items = Vec::new();
    let mut identities = Vec::new();
    for (dispatch, output) in dispatches.iter().zip(outputs) {
        let operation = dispatch.operation.replace('-', "");
        let mut item = serde_json::json!({"operation_id":operation,"attempt_id":dispatch.attempt,
            "request":provider::dispatch_payload(dispatch, *output)?});
        // Structured output stays whole until structured deltas are verified.
        if deltas
            && dispatch.decisions.is_none()
            && matches!(output, provider::RequestOutput::Prose)
        {
            item["deltas"] = serde_json::json!(true);
        }
        items.push(item);
        identities.push((operation, dispatch.attempt.clone()));
    }
    let body = serde_json::to_vec(&serde_json::json!({"version": 3, "items": items}))?;
    if body.len() > 1024 * 1024 {
        return Err(AppError::new(
            ErrorCode::Validation,
            "Operation group exceeds its input limit.",
        ));
    }
    crate::ai::content::capture::request("text", "application/json", &body, key)?;
    let request = client
        .post(&first.target.url)
        .header(reqwest::header::CONTENT_TYPE, "application/json")
        .header(reqwest::header::ACCEPT, "application/x-ndjson")
        .body(body);
    let request = if key.is_empty() {
        request
    } else {
        request.bearer_auth(key)
    };
    let request = if first.route == crate::model::ConnectionRoute::Hosted {
        crate::ai::hosted::identity(request, &first.install_id)
    } else {
        request
    };
    let mut response = request
        .send()
        .await
        .map_err(|error| network(&error, "grouped_request"))?;
    if !response.status().is_success() {
        if let Some(message) = crate::ai::connections::auth_errors::message(
            first.route,
            response.url().as_str(),
            "Chat",
            response.status().as_u16(),
        ) {
            crate::ai::content::capture::rejected(response).await?;
            return Err(AppError::new(ErrorCode::Provider, message));
        }
        if response.status().is_server_error() {
            crate::ai::content::capture::rejected(response).await?;
            return Err(unknown());
        }
        return Err(crate::ai::hosted::body(response)
            .await
            .err()
            .unwrap_or_else(unknown));
    }
    if response
        .headers()
        .get(reqwest::header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.split(';').next())
        != Some("application/x-ndjson")
    {
        crate::ai::content::capture::rejected(response).await?;
        return Err(unknown());
    }
    let mut decoder = if deltas {
        Decoder::with_deltas(identities.clone())?
    } else {
        Decoder::new(identities.clone())?
    };
    let index = |operation: &str| {
        identities
            .iter()
            .position(|(id, _)| id == operation)
            .ok_or_else(unknown)
    };
    while let Some(chunk) = response
        .chunk()
        .await
        .map_err(|error| network(&error, "grouped_response"))?
    {
        crate::ai::content::capture::chunk(&chunk)?;
        decoder.push_with_deltas(
            &chunk,
            |operation, result| publish(index(operation)?, result),
            |operation, text| {
                on_delta(index(operation)?, text);
                Ok(())
            },
        )?;
    }
    crate::ai::content::capture::complete()?;
    decoder.finish()
}

/// Validate the current frame protocol before inference admission.
pub async fn supports_deltas(
    client: &reqwest::Client,
    key: &str,
    dispatch: &crate::ai::transport::text_request::TextRequest,
) -> Result<bool> {
    #[cfg(feature = "workflow-capture")]
    {
        super::workflow::text::protocol(client, key, dispatch).await
    }
    #[cfg(not(feature = "workflow-capture"))]
    {
        supports_deltas_live(client, key, dispatch).await
    }
}

pub(super) async fn supports_deltas_live(
    client: &reqwest::Client,
    key: &str,
    dispatch: &crate::ai::transport::text_request::TextRequest,
) -> Result<bool> {
    let Some(base) = dispatch.target.url.strip_suffix("/operations") else {
        return Ok(false);
    };
    let request = client.get(format!("{base}/protocol"));
    let request = if key.is_empty() {
        request
    } else {
        request.bearer_auth(key)
    };
    let request = if dispatch.route == crate::model::ConnectionRoute::Hosted {
        crate::ai::hosted::identity(request, &dispatch.install_id)
    } else {
        request
    };
    #[cfg(feature = "workflow-capture")]
    crate::ai::transport::workflow::require_live_network()?;
    let response = tokio::time::timeout(std::time::Duration::from_secs(5), request.send())
        .await
        .map_err(|_| protocol_timeout())?
        .map_err(|error| network(&error, "grouped_protocol"))?;
    if !response.status().is_success() {
        return Ok(false);
    }
    let bytes = tokio::time::timeout(std::time::Duration::from_secs(5), response.bytes())
        .await
        .map_err(|_| protocol_timeout())?
        .map_err(|error| network(&error, "grouped_protocol"))?;
    if bytes.len() > 64 * 1024 {
        return Ok(false);
    }
    let value: serde_json::Value = serde_json::from_slice(&bytes).unwrap_or_default();
    if !value["operations_versions"]
        .as_array()
        .is_some_and(|v| v.iter().any(|v| v == 3))
    {
        return Err(AppError::new(
            ErrorCode::Provider,
            "This app requires inference protocol version 3. Update the SkellySpeak server before generating a reply.",
        ));
    }
    Ok(true)
}

pub fn compatible(
    a: &crate::ai::transport::text_request::TextRequest,
    b: &crate::ai::transport::text_request::TextRequest,
) -> bool {
    a.decisions.is_some() == b.decisions.is_some()
        && a.route == b.route
        && a.target.url == b.target.url
        && a.credential == b.credential
        && a.target.revision == b.target.revision
        && a.install_id == b.install_id
}

pub async fn complete(
    client: &reqwest::Client,
    key: &str,
    dispatch: &crate::ai::transport::text_request::TextRequest,
) -> Result<Completion> {
    let mut result = None;
    request(client, key, std::slice::from_ref(dispatch), |_, value| {
        result = Some(value);
        Ok(())
    })
    .await?;
    result.ok_or_else(unknown)?
}

pub async fn complete_with_output(
    client: &reqwest::Client,
    key: &str,
    dispatch: &crate::ai::transport::text_request::TextRequest,
    output: provider::RequestOutput<'_>,
) -> Result<Completion> {
    let mut result = None;
    request_with_outputs(
        client,
        key,
        std::slice::from_ref(dispatch),
        &[output],
        |_, value| {
            result = Some(value);
            Ok(())
        },
    )
    .await?;
    result.ok_or_else(unknown)?
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn provider_failures_identify_upstream_without_echoing_remote_text() {
        for (code, expected) in [
            ("OPENROUTER_HTTP_404", "OpenRouter HTTP 404"),
            ("GROQ_HTTP_429", "Groq HTTP 429"),
            ("GROQ_HTTP_401", "service's API credentials"),
        ] {
            let mut decoder = Decoder::new([("one".into(), "a".into())]).unwrap();
            let event = serde_json::json!({"type":"error","operation_id":"one","attempt_id":"a","code":code,"status":502});
            let bytes = format!("{event}\n{{\"type\":\"complete\",\"count\":1}}\n");
            decoder
                .push(bytes.as_bytes(), |_, result| {
                    let error = result.unwrap_err();
                    assert_eq!(error.code, ErrorCode::UnknownOutcome);
                    assert!(error.message.contains(expected));
                    assert!(error.message.contains("no automatic retry"));
                    assert!(error.refusal.is_none());
                    Ok(())
                })
                .unwrap();
            decoder.finish().unwrap();
        }
    }

    fn response(operation: &str, attempt: &str) -> Vec<u8> {
        let mut bytes = serde_json::to_vec(&serde_json::json!({"type":"result", "operation_id":operation,"attempt_id":attempt,
            "response":{"id":"provider","model":"model","choices":[{"finish_reason":"stop","message":{"content":"¡Hola!"}}],"usage":{"prompt_tokens":2,"completion_tokens":3}}})).unwrap();
        bytes.push(b'\n');
        bytes
    }
    #[test]
    fn item_failure_diagnostics_distinguish_server_status_from_bad_completion() {
        for (event, expected) in [
            (
                serde_json::json!({"type":"error","operation_id":"one","attempt_id":"a","code":"REQUEST_REJECTED","status":502}),
                "HTTP 502",
            ),
            (
                serde_json::json!({"type":"result","operation_id":"one","attempt_id":"a","response":{"private":"must not be shown"}}),
                "Invalid completion response",
            ),
        ] {
            let mut decoder = Decoder::new([("one".into(), "a".into())]).unwrap();
            let bytes = format!("{event}\n{{\"type\":\"complete\",\"count\":1}}\n");
            let mut errors = Vec::new();
            decoder
                .push(bytes.as_bytes(), |_, result| {
                    errors.push(result.unwrap_err());
                    Ok(())
                })
                .unwrap();
            decoder.finish().unwrap();
            assert_eq!(errors.len(), 1);
            assert_eq!(errors[0].code, ErrorCode::UnknownOutcome);
            assert!(errors[0].message.contains(expected));
            assert!(!errors[0].message.contains("private"));
            assert!(!errors[0].message.contains("must not be shown"));
        }
    }
    #[test]
    fn partial_results_publish_before_eof_and_survive_missing_siblings() {
        let mut decoder =
            Decoder::new([("one".into(), "a".into()), ("two".into(), "b".into())]).unwrap();
        let mut published = Vec::new();
        for byte in response("two", "b") {
            decoder
                .push(&[byte], |id, value| {
                    published.push((id.to_owned(), value.unwrap().text));
                    Ok(())
                })
                .unwrap();
        }
        assert_eq!(published, [("two".into(), "¡Hola!".into())]);
        assert_eq!(
            decoder.finish().unwrap_err().code,
            ErrorCode::UnknownOutcome
        );
    }
    #[test]
    fn identities_duplicates_and_completion_are_strict() {
        for bad in [
            response("other", "a"),
            response("one", "other"),
            b"{\"type\":\"complete\",\"count\":1}\n".to_vec(),
        ] {
            let mut decoder = Decoder::new([("one".into(), "a".into())]).unwrap();
            assert!(
                decoder
                    .push(&bad, |_, _| panic!("must not publish"))
                    .is_err()
            );
        }
        let mut decoder = Decoder::new([("one".into(), "a".into())]).unwrap();
        decoder.push(&response("one", "a"), |_, _| Ok(())).unwrap();
        assert!(
            decoder
                .push(&response("one", "a"), |_, _| panic!(
                    "duplicate publication"
                ))
                .is_err()
        );
        let mut decoder = Decoder::new([("one".into(), "a".into())]).unwrap();
        decoder.push(&response("one", "a"), |_, _| Ok(())).unwrap();
        decoder
            .push(b"{\"type\":\"complete\",\"count\":1}\n", |_, _| panic!())
            .unwrap();
        decoder.finish().unwrap();
    }
    #[tokio::test]
    async fn hosted_transport_sends_persisted_identity_and_requires_complete_stream() {
        use tokio::io::{AsyncReadExt, AsyncWriteExt};
        for (route, truncated, structured) in [
            (crate::model::ConnectionRoute::Hosted, false, false),
            (crate::model::ConnectionRoute::Hosted, false, true),
            (crate::model::ConnectionRoute::Hosted, true, false),
            (crate::model::ConnectionRoute::Hosted, true, true),
            (crate::model::ConnectionRoute::Custom, false, false),
            (crate::model::ConnectionRoute::Custom, false, true),
            (crate::model::ConnectionRoute::Custom, true, false),
            (crate::model::ConnectionRoute::Custom, true, true),
        ] {
            let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
            let url = format!("http://{}/v1/operations", listener.local_addr().unwrap());
            let conversation_messages = serde_json::json!([
                {"role":"system","content":"Converse naturally in Spanish."},
                {"role":"assistant","content":"Hola. ¿Te gusta la música?"},
                {"role":"user","content":"No, no me gusta música."}
            ]);
            let helper_messages = serde_json::json!([
                {"role":"system","content":"Return a JSON description of the supplied word."},
                {"role":"user","content":"árbol"}
            ]);
            let expected_conversation = conversation_messages.clone();
            let expected_helper = helper_messages.clone();
            let server = tokio::spawn(async move {
                let (mut socket, _) = listener.accept().await.unwrap();
                let mut input = Vec::new();
                loop {
                    let mut chunk = [0u8; 4096];
                    let read = socket.read(&mut chunk).await.unwrap();
                    assert!(read > 0);
                    input.extend_from_slice(&chunk[..read]);
                    if let Some(start) = input.windows(4).position(|w| w == b"\r\n\r\n") {
                        let headers = String::from_utf8_lossy(&input[..start]);
                        let length: usize = headers
                            .lines()
                            .find_map(|l| {
                                l.to_ascii_lowercase()
                                    .strip_prefix("content-length: ")
                                    .map(str::to_owned)
                            })
                            .unwrap()
                            .parse()
                            .unwrap();
                        if input.len() < start + 4 + length {
                            continue;
                        }
                        assert!(headers.starts_with("POST /v1/operations "));
                        assert_eq!(
                            headers
                                .to_ascii_lowercase()
                                .contains("x-skellyspeak-install:"),
                            route == crate::model::ConnectionRoute::Hosted
                        );
                        let payload: serde_json::Value =
                            serde_json::from_slice(&input[start + 4..]).unwrap();
                        assert_eq!(payload["version"], 3);
                        let child = &payload["items"][0]["request"];
                        assert!(child.get("provider").is_none());
                        assert_eq!(child["temperature"], if structured { 0.7 } else { 1.1 });
                        assert_eq!(
                            child["max_tokens"],
                            if structured {
                                provider::GLOSS_OUTPUT_TOKENS
                            } else {
                                provider::MAX_OUTPUT_TOKENS
                            }
                        );
                        if structured {
                            assert_eq!(payload["items"].as_array().unwrap().len(), 2);
                            assert_eq!(child["messages"], expected_helper);
                            assert_eq!(
                                payload["items"][1]["request"]["messages"],
                                expected_conversation
                            );
                            assert!(
                                payload["items"][1]["request"]
                                    .get("response_format")
                                    .is_none()
                            );
                            assert_eq!(
                                child["response_format"],
                                serde_json::json!({"type":"json_schema","json_schema":{"name":"fixture","strict":true,"schema":{"type":"object"}}})
                            );
                        } else {
                            assert_eq!(child["messages"], expected_conversation);
                            assert!(child.get("response_format").is_none());
                        }
                        assert_eq!(
                            payload["items"][0]["attempt_id"],
                            "2000000000-aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"
                        );
                        break;
                    }
                }
                let mut body = response(
                    "11111111111111111111111111111111",
                    "2000000000-aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
                );
                if structured {
                    let mut second = response(
                        "22222222222222222222222222222222",
                        "2000000000-bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb",
                    );
                    let mut first: serde_json::Value = serde_json::from_slice(&body).unwrap();
                    first["response"]["choices"][0]["message"]["content"] =
                        serde_json::json!("{\"same\":1,\"same\":2}");
                    first["response"]["choices"][0]["finish_reason"] = serde_json::json!("length");
                    second.extend(serde_json::to_vec(&first).unwrap());
                    second.push(b'\n');
                    body = second;
                }
                if !truncated {
                    body.extend(serde_json::to_vec(&serde_json::json!({"type":"complete", "count":if structured {2} else {1}})).unwrap());
                    body.push(b'\n');
                }
                let headers = format!(
                    "HTTP/1.1 200 OK\r\nContent-Type: application/x-ndjson\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                    body.len()
                );
                socket.write_all(headers.as_bytes()).await.unwrap();
                socket.write_all(&body).await.unwrap();
            });
            let dispatch = crate::ai::transport::text_request::TextRequest {
                temperature: if structured { 0.7 } else { 1.1 },

                decisions: None,

                target: crate::ai::connections::access::ResolvedTarget {
                    audio_resolution: None,
                    route,
                    revision: 1,
                    url,
                    model: "google/gemini-2.5-flash".into(),
                    credential: Some("opaque".into()),
                },
                attempt: "2000000000-aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa".into(),
                operation: "11111111-1111-1111-1111-111111111111".into(),
                credential: "opaque".into(),
                model: "google/gemini-2.5-flash".into(),
                messages: serde_json::from_value(if structured {
                    helper_messages
                } else {
                    conversation_messages.clone()
                })
                .unwrap(),
                route,
                install_id: "test".into(),
            };
            let schema = serde_json::json!({"type":"object"});
            if structured {
                let second = crate::ai::transport::text_request::TextRequest {
                    temperature: 0.7,

                    decisions: None,

                    target: dispatch.target.clone(),
                    operation: "22222222-2222-2222-2222-222222222222".into(),
                    attempt: "2000000000-bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb".into(),
                    credential: dispatch.credential.clone(),
                    model: dispatch.model.clone(),
                    messages: serde_json::from_value(conversation_messages).unwrap(),
                    route,
                    install_id: dispatch.install_id.clone(),
                };
                let mut arrived = Vec::new();
                let result = request_with_outputs(
                    &provider::client().unwrap(),
                    "test-only",
                    &[dispatch, second],
                    &[
                        provider::RequestOutput::JsonSchema {
                            max_output_tokens: provider::GLOSS_OUTPUT_TOKENS,
                            name: "fixture",
                            schema: &schema,
                        },
                        provider::RequestOutput::Prose,
                    ],
                    |index, result| {
                        let completion = result.unwrap();
                        assert_eq!(
                            (completion.input_tokens, completion.output_tokens),
                            (Some(2), Some(3))
                        );
                        if index == 0 {
                            assert_eq!(completion.text, "{\"same\":1,\"same\":2}");
                            assert_eq!(completion.finish_reason, "length");
                        } else {
                            assert_eq!(completion.text, "¡Hola!");
                        }
                        arrived.push(index);
                        Ok(())
                    },
                )
                .await;
                assert_eq!(arrived, vec![1, 0]);
                if truncated {
                    assert_eq!(result.unwrap_err().code, ErrorCode::UnknownOutcome);
                } else {
                    result.unwrap();
                }
            } else {
                let result =
                    provider::complete(&provider::client().unwrap(), "test-only", &dispatch).await;
                if truncated {
                    assert_eq!(result.unwrap_err().code, ErrorCode::UnknownOutcome);
                } else {
                    assert_eq!(result.unwrap().text, "¡Hola!");
                }
            }
            server.await.unwrap();
        }
    }

    #[test]
    fn exhausted_server_retries_are_reported_without_claiming_no_retry() {
        let mut decoder = Decoder::new([("one".into(), "a".into())]).unwrap();
        let event = serde_json::json!({"type":"error", "operation_id":"one", "attempt_id":"a", "code":"OPENROUTER_HTTP_429", "status":502,
            "diagnostics":{"automatic_retries":[{"number":1,"delay_ms":1000}], "http":{"status":429}}});
        decoder
            .push(format!("{event}\n").as_bytes(), |_, result| {
                let error = result.unwrap_err();
                assert!(error.message.contains("retries scheduled: 1"));
                assert!(!error.message.contains("no automatic retry"));
                assert_eq!(
                    error.diagnostics.unwrap()["automatic_retries"][0]["delay_ms"],
                    1000
                );
                Ok(())
            })
            .unwrap();
    }

    #[test]
    fn daily_refusal_retains_specific_limit_and_inspection_metadata() {
        let mut decoder = Decoder::new([("one".into(), "a".into())]).unwrap();
        let event = serde_json::json!({"type":"error","operation_id":"one","attempt_id":"a","code":"SHARED_ACCOUNT_DAILY_LIMIT","status":429,"request_id":"0123456789abcdef0123456789abcdef"});
        decoder
            .push(format!("{event}\n").as_bytes(), |_, result| {
                let error = result.unwrap_err();
                assert!(
                    error
                        .message
                        .contains("service's daily authenticated-request limit")
                );
                assert!(matches!(
                    error.refusal.as_ref().unwrap().reason,
                    crate::model::RefusalReason::DailyLimit
                ));
                let details = error.diagnostics.unwrap();
                assert_eq!(details["code"], "SHARED_ACCOUNT_DAILY_LIMIT");
                assert_eq!(details["status"], 429);
                assert_eq!(details["request_id"], "0123456789abcdef0123456789abcdef");
                Ok(())
            })
            .unwrap();
    }

    #[test]
    fn oversized_line_and_structured_refusal() {
        let mut decoder = Decoder::new([("one".into(), "a".into())]).unwrap();
        assert!(
            decoder
                .push(&vec![b'x'; LINE_LIMIT + 1], |_, _| panic!())
                .is_err()
        );
        let mut decoder = Decoder::new([("one".into(), "a".into())]).unwrap();
        decoder.push(b"{\"type\":\"error\",\"operation_id\":\"one\",\"attempt_id\":\"a\",\"code\":\"ACCOUNT_INFLIGHT_LIMIT\",\"status\":429,\"retry_after\":5}\n", |_, result| {
            assert!(result.unwrap_err().refusal.unwrap().retry_at.is_some()); Ok(())
        }).unwrap();
    }
}

#[cfg(test)]
mod delta_tests {
    use super::*;

    fn line(value: serde_json::Value) -> Vec<u8> {
        let mut bytes = serde_json::to_vec(&value).unwrap();
        bytes.push(b'\n');
        bytes
    }
    fn delta(offset: usize, text: &str) -> Vec<u8> {
        line(
            serde_json::json!({"type":"frames","operation_id":"one","attempt_id":"a","offset":offset,"frames":[{"choices":[{"delta":{"content":text}}]}]}),
        )
    }
    fn result() -> Vec<u8> {
        line(
            serde_json::json!({"type":"result","operation_id":"one","attempt_id":"a",
            "response":{"id":"p","model":"m","choices":[{"finish_reason":"stop","message":{"content":"¡Qué 𠮷!"}}]}}),
        )
    }
    fn feed(
        decoder: &mut Decoder,
        bytes: &[u8],
        texts: &mut Vec<String>,
        published: &mut Vec<String>,
    ) -> Result<()> {
        decoder.push_with_deltas(
            bytes,
            |_, value| {
                published.push(value?.text);
                Ok(())
            },
            |_, text| {
                texts.push(text.to_owned());
                Ok(())
            },
        )
    }

    #[test]
    fn unrequested_frames_are_a_protocol_failure() {
        let mut decoder = Decoder::new([("one".into(), "a".into())]).unwrap();
        let error = decoder.push(&delta(0, "Hola"), |_, _| Ok(())).unwrap_err();
        assert_eq!(error.code, ErrorCode::UnknownOutcome);
    }

    #[test]
    fn frames_accumulate_by_frame_offset_and_never_publish_a_partial_result() {
        let mut decoder = Decoder::with_deltas([("one".into(), "a".into())]).unwrap();
        let (mut texts, mut published) = (Vec::new(), Vec::new());
        // "𠮷" is outside the BMP: one scalar value, two UTF-16 units.
        for bytes in [delta(0, "¡Qué "), delta(1, "𠮷"), delta(2, "!")] {
            for byte in bytes {
                feed(&mut decoder, &[byte], &mut texts, &mut published).unwrap();
            }
        }
        assert_eq!(texts, ["¡Qué ", "¡Qué 𠮷", "¡Qué 𠮷!"]);
        assert!(published.is_empty(), "deltas never publish or end an item");
        feed(&mut decoder, &result(), &mut texts, &mut published).unwrap();
        feed(
            &mut decoder,
            &line(serde_json::json!({"type":"complete","count":1})),
            &mut texts,
            &mut published,
        )
        .unwrap();
        assert_eq!(published, ["¡Qué 𠮷!"]);
        decoder.finish().unwrap();
    }

    #[test]
    fn gaps_overlaps_late_deltas_and_oversize_text_are_protocol_failures() {
        for bytes in [
            [delta(0, "Ho"), delta(3, "la")].concat(),
            [delta(0, "Ho"), delta(0, "la")].concat(),
            [delta(0, "Ho"), result(), delta(2, "la")].concat(),
            line(
                serde_json::json!({"type":"frames","operation_id":"one","attempt_id":"other","offset":0,"frames":[{"choices":[{"delta":{"content":"x"}}]}]}),
            ),
            delta(0, &"a".repeat(RESPONSE_TEXT_LIMIT + 1)),
        ] {
            let mut decoder = Decoder::with_deltas([("one".into(), "a".into())]).unwrap();
            let (mut texts, mut published) = (Vec::new(), Vec::new());
            let error = feed(&mut decoder, &bytes, &mut texts, &mut published).unwrap_err();
            assert_eq!(error.code, ErrorCode::UnknownOutcome);
        }
    }

    async fn protocol_server(body: &'static str) -> String {
        use tokio::io::{AsyncReadExt, AsyncWriteExt};
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let url = format!("http://{}/v1/operations", listener.local_addr().unwrap());
        tokio::spawn(async move {
            let (mut socket, _) = listener.accept().await.unwrap();
            let mut input = [0u8; 4096];
            let read = socket.read(&mut input).await.unwrap();
            assert!(String::from_utf8_lossy(&input[..read]).starts_with("GET /v1/protocol "));
            let response = format!(
                "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                body.len()
            );
            socket.write_all(response.as_bytes()).await.unwrap();
        });
        url
    }
    fn dispatch(url: String) -> crate::ai::transport::text_request::TextRequest {
        crate::ai::transport::text_request::TextRequest {
            temperature: 0.7,
            target: crate::ai::connections::access::ResolvedTarget {
                audio_resolution: None,
                route: crate::model::ConnectionRoute::Custom,
                revision: 1,
                url,
                model: "m".into(),
                credential: None,
            },
            attempt: "a".into(),
            operation: "o".into(),
            credential: String::new(),
            model: "m".into(),
            route: crate::model::ConnectionRoute::Custom,
            install_id: "install".into(),
            messages: vec![provider::PromptMessage {
                role: "user".into(),
                content: "Hola".into(),
            }],

            decisions: None,
        }
    }

    #[tokio::test]
    async fn only_a_server_advertising_version_3_gets_frames() {
        let client = reqwest::Client::new();
        let modern =
            protocol_server(r#"{"protocol":"skellyspeak","version":1,"operations_versions":[3]}"#)
                .await;
        assert!(
            supports_deltas(&client, "", &dispatch(modern))
                .await
                .unwrap()
        );
        let older = protocol_server(r#"{"protocol":"skellyspeak","version":1}"#).await;
        assert!(
            supports_deltas(&client, "", &dispatch(older))
                .await
                .is_err()
        );
        assert!(
            !supports_deltas(
                &client,
                "",
                &dispatch("http://127.0.0.1:9/chat/completions".into())
            )
            .await
            .unwrap()
        );
        assert!(
            supports_deltas(
                &client,
                "",
                &dispatch("http://127.0.0.1:9/v1/operations".into())
            )
            .await
            .is_err(),
            "unreachable: retried later, never cached"
        );
    }

    #[tokio::test]
    async fn requests_frames_for_prose_only_and_reports_text_before_the_result() {
        use tokio::io::{AsyncReadExt, AsyncWriteExt};
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let url = format!("http://{}/v1/operations", listener.local_addr().unwrap());
        let server = tokio::spawn(async move {
            let (mut socket, _) = listener.accept().await.unwrap();
            let mut input = Vec::new();
            let payload = loop {
                let mut chunk = [0u8; 4096];
                let read = socket.read(&mut chunk).await.unwrap();
                input.extend_from_slice(&chunk[..read]);
                if let Some(start) = input.windows(4).position(|w| w == b"\r\n\r\n") {
                    let headers = String::from_utf8_lossy(&input[..start]).to_ascii_lowercase();
                    let length: usize = headers
                        .lines()
                        .find_map(|l| l.strip_prefix("content-length: ").map(str::to_owned))
                        .unwrap()
                        .parse()
                        .unwrap();
                    if input.len() >= start + 4 + length {
                        break serde_json::from_slice::<serde_json::Value>(
                            &input[start + 4..start + 4 + length],
                        )
                        .unwrap();
                    }
                }
            };
            let prose = payload["items"][0]["operation_id"]
                .as_str()
                .unwrap()
                .to_owned();
            let structured = payload["items"][1]["operation_id"]
                .as_str()
                .unwrap()
                .to_owned();
            let body = [
                serde_json::json!({"type":"frames","operation_id":prose,"attempt_id":"a1","offset":0,"frames":[{"choices":[{"delta":{"content":"¡Ho"}}]}]}),
                serde_json::json!({"type":"frames","operation_id":prose,"attempt_id":"a1","offset":1,"frames":[{"choices":[{"delta":{"content":"la!"}}]}]}),
                serde_json::json!({"type":"result","operation_id":prose,"attempt_id":"a1","response":{"id":"p","model":"m","choices":[{"finish_reason":"stop","message":{"content":"¡Hola!"}}]}}),
                serde_json::json!({"type":"result","operation_id":structured,"attempt_id":"a2","response":{"id":"p","model":"m","choices":[{"finish_reason":"stop","message":{"content":"{}"}}]}}),
                serde_json::json!({"type":"complete","count":2}),
            ].iter().map(|event| format!("{event}\n")).collect::<String>();
            let response = format!(
                "HTTP/1.1 200 OK\r\nContent-Type: application/x-ndjson\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                body.len()
            );
            socket.write_all(response.as_bytes()).await.unwrap();
            payload
        });
        let mut first = dispatch(url.clone());
        first.attempt = "a1".into();
        first.operation = "11111111-1111-1111-1111-111111111111".into();
        let mut second = dispatch(url);
        second.attempt = "a2".into();
        second.operation = "22222222-2222-2222-2222-222222222222".into();
        let schema = serde_json::json!({"type":"object"});
        let outputs = [
            provider::RequestOutput::Prose,
            provider::RequestOutput::JsonSchema {
                max_output_tokens: 2048,
                name: "fixture",
                schema: &schema,
            },
        ];
        let mut events = Vec::new();
        let seen = std::cell::RefCell::new(&mut events);
        request_streaming(
            &reqwest::Client::new(),
            "",
            &[first, second],
            &outputs,
            true,
            |index, outcome| {
                seen.borrow_mut()
                    .push(format!("result {index} {}", outcome?.text));
                Ok(())
            },
            |index, text| seen.borrow_mut().push(format!("delta {index} {text}")),
        )
        .await
        .unwrap();
        assert_eq!(
            events,
            [
                "delta 0 ¡Ho",
                "delta 0 ¡Hola!",
                "result 0 ¡Hola!",
                "result 1 {}"
            ]
        );
        let payload = server.await.unwrap();
        assert_eq!(payload["version"], 3);
        assert_eq!(payload["items"][0]["deltas"], true);
        assert!(
            payload["items"][1].get("deltas").is_none(),
            "structured output stays whole"
        );
    }
}
