use super::PromptMessage;
use crate::model::{AppError, ConnectionRoute, ErrorCode, Result};

pub const MAX_OUTPUT_TOKENS: i32 = 2048;
pub const GLOSS_OUTPUT_TOKENS: i32 = 8192;

pub fn payload(
    model: &str,
    messages: &[PromptMessage],
    route: ConnectionRoute,
) -> Result<serde_json::Value> {
    let payload = serde_json::json!({"model":model,"messages":messages,"stream":false,"max_tokens":MAX_OUTPUT_TOKENS,"temperature":0.7,"reasoning":{"enabled":false}});
    let _ = route;
    Ok(payload)
}
/// Transport-local request contract; never inferred from prompt text or serialized over IPC.
#[derive(Clone, Copy, Debug)]
pub enum RequestOutput<'a> {
    Prose,
    JsonSchema {
        max_output_tokens: i32,
        name: &'a str,
        schema: &'a serde_json::Value,
    },
}

/// The structured output contract for task schemas (translation, coaching and
/// other JSON tasks), shared by every engine that sends them.
pub fn structured_output(schema: &serde_json::Value) -> RequestOutput<'_> {
    RequestOutput::JsonSchema {
        max_output_tokens: 2048,
        name: "coaching",
        schema,
    }
}

const STRUCTURED_INPUT_LIMIT: usize = 100_000;
const SERVER_INPUT_OVERHEAD: usize = 1024;

fn structured_error(message: &str) -> AppError {
    AppError::new(ErrorCode::Validation, message)
}

/// Bound Python json.dumps(ensure_ascii=False) size, including its default spaces.
/// Float spellings can differ between runtimes; reserve 16 extra bytes per float.
/// This is deliberately a conservative byte bound, not a token count.
fn structured_input_bound(value: &serde_json::Value) -> Result<usize> {
    use serde_json::Value;
    let mut pending = vec![(value, 0usize)];
    let mut bytes = SERVER_INPUT_OVERHEAD;
    while let Some((value, depth)) = pending.pop() {
        if depth > 64 {
            return Err(structured_error(
                "Structured request exceeds the nesting limit.",
            ));
        }
        let size = match value {
            Value::Array(items) => {
                pending.extend(items.iter().map(|item| (item, depth + 1)));
                2 + items.len().saturating_sub(1) * 2
            }
            Value::Object(fields) => {
                let mut size = 2 + fields.len().saturating_sub(1) * 2;
                for (key, value) in fields {
                    if key.len() > STRUCTURED_INPUT_LIMIT {
                        return Err(structured_error(
                            "Structured request exceeds its input limit.",
                        ));
                    }
                    size += serde_json::to_vec(key)?.len() + 2;
                    pending.push((value, depth + 1));
                }
                size
            }
            Value::String(text) => {
                if text.len() > STRUCTURED_INPUT_LIMIT {
                    return Err(structured_error(
                        "Structured request exceeds its input limit.",
                    ));
                }
                serde_json::to_vec(text)?.len()
            }
            Value::Number(number) => {
                number.to_string().len() + if number.is_f64() { 16 } else { 0 }
            }
            Value::Bool(true) => 4,
            Value::Bool(false) => 5,
            Value::Null => 4,
        };
        bytes = bytes
            .checked_add(size)
            .ok_or_else(|| structured_error("Structured request exceeds its input limit."))?;
        if bytes > STRUCTURED_INPUT_LIMIT {
            return Err(structured_error(
                "Structured request exceeds its input limit.",
            ));
        }
    }
    Ok(bytes)
}

pub fn payload_with_output(
    model: &str,
    messages: &[PromptMessage],
    route: ConnectionRoute,
    output: RequestOutput<'_>,
) -> Result<serde_json::Value> {
    let RequestOutput::JsonSchema {
        name,
        schema,
        max_output_tokens,
    } = output
    else {
        return payload(model, messages, route);
    };
    if !(1..=32768).contains(&max_output_tokens) {
        return Err(structured_error(
            "Structured output token limit must be between 1 and 32768.",
        ));
    }
    // The server accepts a strict named schema object. Restrict names to portable
    // identifiers locally; nested schema keyword support remains provider-owned.
    if name.is_empty()
        || name.len() > 64
        || !name
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"_-".contains(&b))
        || !schema.is_object()
    {
        return Err(structured_error(
            "Structured output requires a valid schema name and schema object.",
        ));
    }
    if messages.is_empty()
        || messages.len() > 64
        || messages
            .iter()
            .any(|m| !matches!(m.role.as_str(), "system" | "user" | "assistant"))
    {
        return Err(structured_error(
            "Structured requests require 1–64 text messages with supported roles.",
        ));
    }
    // Reject already-oversized raw inputs before the payload builder clones them.
    let raw_bytes = messages.iter().try_fold(model.len(), |total, message| {
        total.checked_add(message.content.len())
    });
    if raw_bytes.is_none_or(|bytes| bytes > STRUCTURED_INPUT_LIMIT) {
        return Err(structured_error(
            "Structured request exceeds its input limit.",
        ));
    }
    // Check depth/size before cloning or serializing a caller-supplied schema.
    structured_input_bound(schema)?;
    let mut request = payload(model, messages, route)?;
    request["max_tokens"] = serde_json::json!(max_output_tokens);
    request["response_format"] = serde_json::json!({"type":"json_schema","json_schema":{"name":name,"strict":true,"schema":schema}});

    structured_input_bound(&request)?;
    Ok(request)
}

#[cfg(test)]
#[path = "tests/payload.rs"]
mod tests;

/// Actual dispatch settings shared by streamed, direct and grouped requests.
pub fn dispatch_payload(
    dispatch: &crate::ai::transport::text_request::TextRequest,
    output: RequestOutput<'_>,
) -> Result<serde_json::Value> {
    if let Some(decisions) = &dispatch.decisions {
        return Ok(decisions.clone());
    }
    let mut body =
        payload_with_output(&dispatch.model, &dispatch.messages, dispatch.route, output)?;
    validate_temperature(dispatch.temperature)?;
    body["temperature"] = serde_json::json!(dispatch.temperature);
    Ok(body)
}

pub fn validate_temperature(temperature: f64) -> Result<()> {
    if !temperature.is_finite() || !(0.0..=2.0).contains(&temperature) {
        return Err(structured_error(
            "Temperature must be finite and between 0 and 2.",
        ));
    }
    Ok(())
}
