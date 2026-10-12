//! Transport metadata capture for native graph producers. No scheduling or I/O.
//! Callers supply prompt/source/credential strings for exact redaction. Retain
//! the original AppError separately for domain handling; Fault is not its record.
use super::provider::Completion;
use crate::{
    ai::graph::{EvidenceValue, ResponseEvidence, UsageEvidence},
    diagnostics::response,
    model::AppError,
};
use serde_json::{Value, json};
use std::collections::BTreeMap;

fn capture(value: &Value, private: &[&str]) -> ResponseEvidence {
    let safe = response::metadata(value, private);
    ResponseEvidence {
        request_id: safe
            .get("request_id")
            .and_then(Value::as_str)
            .map(str::to_owned),
        requested_model: safe
            .get("requested_model")
            .and_then(Value::as_str)
            .map(str::to_owned),
        actual_model: safe
            .get("actual_model")
            .and_then(Value::as_str)
            .map(str::to_owned),
        finish_reason: safe
            .get("finish_reason")
            .and_then(Value::as_str)
            .map(str::to_owned),
        error_code: safe.get("code").and_then(Value::as_str).map(str::to_owned),
        redacted_reason: safe
            .get("message")
            .and_then(Value::as_str)
            .map(str::to_owned),
        additional: BTreeMap::from([("transport".into(), EvidenceValue::ClassifiedJson(safe))]),
        ..Default::default()
    }
}

/// Safe partial metadata can be observed before terminal completion. This also
/// preserves nested validation, timing, usage and billing provenance on failure.
pub fn metadata(value: &Value, private: &[&str]) -> ResponseEvidence {
    capture(value, private)
}

pub fn completion(
    result: &Completion,
    requested_model: &str,
    private: &[&str],
) -> ResponseEvidence {
    let mut private = private.to_vec();
    private.push(&result.text);
    let mut evidence = capture(
        &json!({
            "request_id": result.provider_id,
            "requested_model": requested_model,
            "actual_model": result.actual_model,
            "finish_reason": result.finish_reason,
            "usage": {"input_tokens":result.input_tokens, "output_tokens":result.output_tokens},
            "diagnostics": result.diagnostics,
        }),
        &private,
    );
    evidence.usage = Some(UsageEvidence {
        input_tokens: result.input_tokens.and_then(|v| v.try_into().ok()),
        output_tokens: result.output_tokens.and_then(|v| v.try_into().ok()),
        total_tokens: None,
        provenance: "provider_completion".into(),
    });
    evidence
}

pub fn failure(error: &AppError, requested_model: &str, private: &[&str]) -> ResponseEvidence {
    capture(
        &json!({
            "requested_model": requested_model,
            "code":error.code,
            "message":error.message,
            "refusal":error.refusal,
            "diagnostics":error.diagnostics,
        }),
        private,
    )
}

#[cfg(test)]
mod tests;
