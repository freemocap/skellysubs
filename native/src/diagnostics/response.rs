//! Bounded response metadata. Content and credentials never enter retained diagnostics.
use crate::model::{AppError, ErrorCode};
use serde_json::{Value, json};
const LIMIT: usize = 32_768;
pub fn scrub(text: &str, private: &[&str]) -> String {
    super::policy::scrub(text, private)
}
fn content(key: &str) -> bool {
    super::policy::sensitive(key)
}
fn public_string(key: &str) -> bool {
    super::policy::get().public_fields.iter().any(|f| f == key)
}
/// Keep the causal spine when optional metadata exceeds the record budget.
fn compact(value: &Value, budget: &mut usize) -> Value {
    if *budget == 0 {
        return json!("[truncated: metadata limit]");
    }
    *budget -= 1;
    match value {
        Value::String(text) if text.chars().count() > 512 => json!(format!(
            "{}[truncated: string limit]",
            text.chars().take(512).collect::<String>()
        )),
        Value::Object(object) => {
            let mut result = serde_json::Map::new();
            for key in [
                "message",
                "code",
                "reason",
                "stage",
                "path",
                "expected",
                "request_id",
                "requestId",
                "model",
                "finish_reason",
                "status",
                "line",
                "column",
                "error",
                "cause",
                "causes",
                "diagnostics",
                "response",
                "usage",
            ] {
                if let Some(value) = object.get(key) {
                    result.insert(key.into(), compact(value, budget));
                }
            }
            // Preserve bounded counters, timings and billing facts after causal fields.
            for (key, value) in object {
                if *budget == 0 {
                    break;
                }
                if !result.contains_key(key)
                    && matches!(value, Value::Number(_) | Value::Bool(_) | Value::Null)
                {
                    result.insert(key.clone(), compact(value, budget));
                }
            }
            result.insert("truncated".into(), json!(true));
            Value::Object(result)
        }
        Value::Array(values) => json!(
            values
                .iter()
                .take(8)
                .map(|v| compact(v, budget))
                .collect::<Vec<_>>()
        ),
        _ => value.clone(),
    }
}

pub fn metadata(value: &Value, private: &[&str]) -> Value {
    fn visit(v: &Value, field: &str, private: &[&str], depth: usize, budget: &mut usize) -> Value {
        if depth > 12 || *budget == 0 {
            return json!("[truncated: metadata limit]");
        }
        *budget -= 1;
        match v {
            Value::Object(map) => {
                let mut result = serde_json::Map::new();
                let mut entries: Vec<_> = map.iter().collect();
                entries.sort_by_key(|(key, _)| match key.as_str() {
                    "error" | "message" | "code" | "stage" | "reason" | "path" | "expected"
                    | "cause" | "causes" | "diagnostics" => 0,
                    "request_id" | "response" | "usage" | "http" => 1,
                    _ => 2,
                });
                for (i, (key, value)) in entries.into_iter().enumerate() {
                    if i == 64 {
                        result.insert("truncated_fields".into(), json!(map.len() - i));
                        break;
                    }
                    let safe_key = key.len() <= 64
                        && key
                            .bytes()
                            .all(|b| b.is_ascii_alphanumeric() || b"_.-".contains(&b))
                        && !private.contains(&key.as_str());
                    if !safe_key {
                        result.insert(
                            format!("redacted_field_{i}"),
                            json!(super::policy::get().content_tag),
                        );
                    } else {
                        result.insert(
                            key.clone(),
                            if content(&key.to_lowercase()) {
                                json!(super::policy::tag(key))
                            } else {
                                visit(value, key, private, depth + 1, budget)
                            },
                        );
                    }
                }
                Value::Object(result)
            }
            Value::Array(items) => {
                let mut out: Vec<_> = items
                    .iter()
                    .take(32)
                    .map(|item| visit(item, field, private, depth + 1, budget))
                    .collect();
                if items.len() > 32 {
                    out.push(json!({"truncated_items":items.len()-32}));
                }
                json!(out)
            }
            Value::String(s) if s.starts_with("[redacted") || s.starts_with("[truncated") => {
                json!(s.chars().take(128).collect::<String>())
            }
            Value::String(s)
                if matches!(field, "id" | "request_id" | "x_request_id" | "requestId")
                    && s.len() <= 128
                    && s.bytes()
                        .all(|b| b.is_ascii_alphanumeric() || b"_.-".contains(&b))
                    && !private.iter().any(|p| !p.is_empty() && s.contains(p))
                    && !["sk-", "gsk_", "eyJ"].iter().any(|p| s.starts_with(p)) =>
            {
                json!(s)
            }
            Value::String(s) if public_string(field) => json!(scrub(s, private)),
            Value::String(_) => json!(super::policy::get().content_tag),
            _ => v.clone(),
        }
    }
    let result = visit(value, "", private, 0, &mut 1024);
    if result.to_string().len() > LIMIT {
        let mut summary = serde_json::Map::new();
        summary.insert("reason".into(), json!("metadata_size_limit"));
        summary.insert("truncated".into(), json!(true));
        // A large metadata attachment must never erase the failure explanation.
        for key in [
            "name",
            "message",
            "detail",
            "reason",
            "request_id",
            "stack",
            "componentStack",
            "code",
            "stage",
        ] {
            if let Some(Value::String(value)) = result.get(key) {
                summary.insert(key.into(), json!(scrub(value, private)));
            }
        }
        for key in [
            "error",
            "cause",
            "causes",
            "diagnostics",
            "response",
            "http",
            "usage",
        ] {
            if let Some(value) = result.get(key) {
                summary.insert(key.into(), compact(value, &mut 24));
            }
        }
        Value::Object(summary)
    } else {
        result
    }
}
pub fn reason(value: &Value) -> Option<&str> {
    for key in [
        "message",
        "detail",
        "error",
        "provider_error",
        "diagnostics",
    ] {
        if let Some(v) = value.get(key) {
            if let Some(s) = v
                .as_str()
                .filter(|s| !s.starts_with("[redacted") && !s.is_empty())
            {
                return Some(s);
            }
            if let Some(s) = reason(v) {
                return Some(s);
            }
        }
    }
    None
}
pub fn invalid(stage: &str, path: &str, expected: &str, value: &Value) -> AppError {
    AppError::new(
        ErrorCode::UnknownOutcome,
        format!(
            "Invalid {stage} response at {path}: expected {expected}. No automatic retry was made."
        ),
    )
    .with_diagnostics(
        json!({"stage":stage,"path":path,"expected":expected,"response":metadata(value, &[])}),
    )
}
pub fn network(error: &reqwest::Error, stage: &str) -> AppError {
    let reason = if error.is_timeout() {
        "timeout"
    } else if error.is_connect() {
        "connection_failed"
    } else if error.is_body() {
        "body_read_failed"
    } else if error.is_decode() {
        "decode_failed"
    } else {
        "transport_failed"
    };
    let mut causes = Vec::new();
    let mut source = std::error::Error::source(error);
    while let Some(cause) = source {
        if causes.len() == 8 {
            break;
        }
        causes.push(serde_json::json!({"message": scrub(&cause.to_string(), &[])}));
        source = cause.source();
    }
    AppError::new(
        ErrorCode::UnknownOutcome,
        format!(
            "{stage}: {reason}. Provider processing may have occurred; no automatic retry was made."
        ),
    )
    .with_diagnostics(json!({"stage":stage,"reason":reason,"message":scrub(&error.to_string(), &[]),"causes":causes}))
}

/// Preserve a caller's error code/recovery policy while retaining transport causes.
pub fn network_context(error: &reqwest::Error, stage: &str, mut failure: AppError) -> AppError {
    let mut context = network(error, stage).diagnostics.unwrap();
    if let Some(previous) = failure.diagnostics {
        context["previous"] = previous;
    }
    failure.diagnostics = Some(context);
    failure
}

/// JSON diagnostics must not echo rejected string values (which may be content).
pub fn json_context(error: &serde_json::Error, stage: &str, failure: AppError) -> AppError {
    let previous = failure.diagnostics.clone();
    failure.with_diagnostics(
        json!({"stage":stage,"line":error.line(),"column":error.column(),
        "reason":previous.as_ref().and_then(|v| v.get("reason")).cloned().unwrap_or_else(|| json!(format!("{:?}", error.classify()))), "category":format!("{:?}", error.classify()),
        "previous":previous, "redaction":"parser prose omitted because it may echo response content"}),
    )
}

/// OS identity without paths, device labels, or arbitrary custom error payloads.
pub fn io_context(error: &std::io::Error, stage: &str, failure: AppError) -> AppError {
    let previous = failure.diagnostics.clone();
    failure.with_diagnostics(json!({"stage":stage,"reason":format!("{:?}", error.kind()),
        "os_code":error.raw_os_error(), "previous":previous, "redaction":"path and custom IO payload omitted"}))
}
pub async fn http_error(
    mut response: reqwest::Response,
    label: &str,
    private: &[&str],
) -> AppError {
    let status = response.status().as_u16();
    let refusal = (status == 429).then(|| crate::ai::policy::refusal::from_response(&response));
    let mut headers = serde_json::Map::new();
    for (key, value) in response.headers() {
        let name = key.as_str();
        if (matches!(
            name,
            "request-id"
                | "x-request-id"
                | "retry-after"
                | "content-type"
                | "processing-ms"
                | "openai-processing-ms"
        ) || name.starts_with("x-ratelimit-")
            || name.starts_with("ratelimit-"))
            && let Ok(value) = value.to_str()
        {
            headers.insert(
                name.replace('-', "_"),
                metadata(&json!({name.replace('-', "_"): value}), private)[name.replace('-', "_")]
                    .clone(),
            );
        }
    }
    let mut body = Vec::new();
    let mut truncated = false;
    let mut unreadable = false;
    let mut read_error = None;
    loop {
        match response.chunk().await {
            Ok(Some(chunk)) => {
                if let Err(error) = crate::ai::content::capture::chunk(&chunk) {
                    return error;
                }
                if body.len() + chunk.len() <= LIMIT {
                    body.extend_from_slice(&chunk);
                } else {
                    truncated = true;
                    break;
                }
            }
            Err(cause) => {
                unreadable = true;
                read_error = network(&cause, "http_error_body").diagnostics;
                break;
            }
            Ok(None) => {
                if let Err(error) = crate::ai::content::capture::complete() {
                    return error;
                }
                break;
            }
        }
    }
    let value = if truncated || unreadable {
        json!({"reason":"error_body_incomplete"})
    } else {
        serde_json::from_slice(&body).unwrap_or_else(
            |_| json!({"reason":"non_json_error_body","detail":String::from_utf8_lossy(&body)}),
        )
    };
    let value = metadata(&value, private);
    let detail = reason(&value).unwrap_or("No readable provider reason was supplied.");
    let mut error = AppError::new(ErrorCode::Provider, format!("{label} HTTP {status}: {detail} No automatic retry was made."))
        .with_diagnostics(json!({"stage":"http","status":status,"response_headers":headers,"response":value,"truncated":truncated,"unreadable":unreadable,"read_error":read_error}));
    error.refusal = refusal;
    error
}
pub fn column(row: &rusqlite::Row<'_>, index: usize) -> rusqlite::Result<Option<Value>> {
    row.get::<_, Option<String>>(index)?
        .map(|s| {
            serde_json::from_str(&s).map_err(|e| {
                rusqlite::Error::FromSqlConversionFailure(
                    index,
                    rusqlite::types::Type::Text,
                    Box::new(e),
                )
            })
        })
        .transpose()
}

/// Shared error envelope used by disk events and persisted attempt receipts.
pub fn error_metadata(error: &AppError, private: &[&str]) -> Value {
    json!({"code":error.code,"message":scrub(&error.message, private),
        "diagnostics":error.diagnostics.as_ref().map(|v| metadata(v, private)),
        "refusal":error.refusal.as_ref().map(|v| metadata(&json!(v), private))})
}

pub fn retained(success: Option<&Value>, error: Option<&AppError>) -> Option<String> {
    retained_with_private(success, error, &[])
}

pub fn retained_with_private(
    success: Option<&Value>,
    error: Option<&AppError>,
    private: &[&str],
) -> Option<String> {
    if success.is_none() && error.is_none() {
        return None;
    }
    Some(json!({"response":success.map(|v| metadata(v, private)),"error":error.map(|e| error_metadata(e, private))}).to_string())
}

pub fn headers(response: &reqwest::Response) -> Value {
    let mut kept = serde_json::Map::new();
    for (key, value) in response.headers() {
        let name = key.as_str();
        if (matches!(
            name,
            "request-id"
                | "x-request-id"
                | "retry-after"
                | "content-type"
                | "processing-ms"
                | "openai-processing-ms"
        ) || name.starts_with("x-ratelimit-")
            || name.starts_with("ratelimit-"))
            && let Ok(value) = value.to_str()
        {
            kept.insert(
                name.replace('-', "_"),
                metadata(&json!({name.replace('-', "_"): value}), &[])[name.replace('-', "_")]
                    .clone(),
            );
        }
    }
    kept.insert(
        "redacted_header_count".into(),
        json!(response.headers().len() - kept.len()),
    );
    json!({"status":response.status().as_u16(),"response_headers":kept})
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn metadata_limits_and_retained_failures_keep_the_error_explanation() {
        let mut value = json!({"message":"Language selection failed",
            "error":{"message":"Quote validation failed", "diagnostics":{"path":"cards[0].quote"}},
            "response":{"request_id":"req-retained", "usage":{"input_tokens":705,"output_tokens":352}}});
        value["extra"] = json!(
            (0..32)
                .map(|_| json!({"message":"detail ".repeat(400)}))
                .collect::<Vec<_>>()
        );
        let result = metadata(&value, &[]);
        assert_eq!(result["message"], "Language selection failed");
        assert_eq!(result["truncated"], true);
        assert_eq!(result["error"]["message"], "Quote validation failed");
        assert_eq!(result["error"]["diagnostics"]["path"], "cards[0].quote");
        assert_eq!(result["response"]["request_id"], "req-retained");
        assert_eq!(result["response"]["usage"]["input_tokens"], 705);
        assert_eq!(result["response"]["usage"]["output_tokens"], 352);
        let error = AppError::new(
            ErrorCode::Internal,
            "Language selection failed; password=private-key",
        );
        let retained = retained(None, Some(&error)).unwrap();
        assert!(retained.contains("Language selection failed"));
        assert!(!retained.contains("private-key"));
    }
    #[test]
    fn retains_error_message_and_stack_while_redacting_sensitive_spans() {
        let result = metadata(
            &json!({
                "message":"Language picker failed: Maximum update depth exceeded; api_key=short-secret; transcript=private sentence",
                "stack":"at LearningPicker (src/features/settings/LanguagePickers.tsx:51:9)\nat render (assets/app.js:22:1)",
                "cause":{"message":"Permission denied", "code":"EACCES"},
                "request_id":"req-123", "prompt":"private prompt"
            }),
            &[],
        );
        assert!(
            result["message"]
                .as_str()
                .unwrap()
                .contains("Maximum update depth exceeded")
        );
        assert!(
            result["stack"]
                .as_str()
                .unwrap()
                .contains("LanguagePickers.tsx:51:9")
        );
        assert_eq!(result["cause"]["message"], "Permission denied");
        assert_eq!(result["request_id"], "req-123");
        for private in ["short-secret", "private sentence", "private prompt"] {
            assert!(!result.to_string().contains(private));
        }
        assert!(!scrub("Invalid token: Bearer abc123", &[]).contains("abc123"));
        assert!(
            !scrub("Parse failed near \"private text\"", &["private text"])
                .contains("private text")
        );
    }
    #[test]
    fn keeps_identifiers_usage_and_extra_numbers_but_removes_content_and_keys() {
        let value = json!({"id":"provider-request", "model":"vendor/model", "usage":{"prompt_tokens":20,"cached_tokens":8},
            "extra":{"latency_ms":42,"new_text":"private unknown string"},
            "error":{"code":"missing_permissions","message":"Missing permission \"text_to_speech\". Echo private sentence arbitrary-key"},
            "choices":[{"finish_reason":"content_filter","message":{"content":"private answer"}}], "api_key":"arbitrary-key"});
        let out = metadata(&value, &["private sentence", "arbitrary-key"]);
        assert_eq!(out["id"], "provider-request");
        assert_eq!(out["usage"]["cached_tokens"], 8);
        assert_eq!(out["extra"]["latency_ms"], 42);
        assert_eq!(out["choices"][0]["finish_reason"], "content_filter");
        assert!(
            out["error"]["message"]
                .as_str()
                .unwrap()
                .contains("text_to_speech")
        );
        for secret in [
            "private sentence",
            "arbitrary-key",
            "private answer",
            "private unknown string",
        ] {
            assert!(!out.to_string().contains(secret));
        }
        assert!(
            out["extra"]["new_text"]
                .as_str()
                .unwrap()
                .contains("user content redacted")
        );
    }
    #[test]
    fn malformed_completion_retains_partial_receipt_and_identifies_contract() {
        let result = crate::ai::transport::provider::decode(br#"{"id":"partial-request","model":"fixture-model","choices":[{"finish_reason":"content_filter","message":{"content":null}}],"usage":{"prompt_tokens":21,"completion_tokens":3}}"#).unwrap_err();
        assert!(result.message.contains("choices[0].message.content"));
        let details = result.diagnostics.unwrap();
        assert_eq!(details["response"]["id"], "partial-request");
        assert_eq!(details["response"]["usage"]["completion_tokens"], 3);
    }
}
