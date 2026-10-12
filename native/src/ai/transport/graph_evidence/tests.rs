use super::*;
use crate::model::ErrorCode;

fn tree(evidence: &ResponseEvidence) -> &Value {
    let EvidenceValue::ClassifiedJson(value) = &evidence.additional["transport"] else {
        panic!("structured metadata required")
    };
    value
}

#[test]
fn completion_preserves_nested_metadata_without_content_or_inferred_cost() {
    let result = Completion {
        diagnostics: Some(
            json!({"usage":{"cost":0.0005,"allowance":2,"cost_basis":"reported"},
            "timings":[12,null,4],"request_id":"request-17", "prompt":"private prompt",
            "extension":"unknown content", "authorization":"private credential",
            "message":"echo private reply"}),
        ),
        text: "private reply".into(),
        finish_reason: "stop".into(),
        actual_model: "actual".into(),
        provider_id: "provider-17".into(),
        input_tokens: Some(12),
        output_tokens: None,
    };
    let evidence = completion(
        &result,
        "requested",
        &["private prompt", "private credential"],
    );
    assert_eq!(evidence.request_id.as_deref(), Some("provider-17"));
    assert_eq!(evidence.actual_model.as_deref(), Some("actual"));
    assert_eq!(evidence.usage.as_ref().unwrap().output_tokens, None);
    assert!(evidence.billing.is_none());
    assert_eq!(tree(&evidence)["diagnostics"]["usage"]["cost"], 0.0005);
    assert_eq!(
        tree(&evidence)["diagnostics"]["timings"],
        json!([12, null, 4])
    );
    let encoded = serde_json::to_string(&evidence).unwrap();
    for secret in [
        "private prompt",
        "private reply",
        "private credential",
        "unknown content",
    ] {
        assert!(!encoded.contains(secret));
    }
    assert!(encoded.contains("redacted"));
}

#[test]
fn malformed_result_retains_causal_and_partial_response_information() {
    let error = AppError::new(
        ErrorCode::UnknownOutcome,
        "Invalid response for private prompt",
    )
    .with_diagnostics(json!({"stage":"decode","path":"choices","expected":"array",
            "response":{"id":"partial-17","model":"actual","usage":{"input_tokens":12},
                "error":{"code":"invalid_shape","message":"echo private prompt"}}}));
    let evidence = failure(&error, "requested", &["private prompt"]);
    let diagnostics = &tree(&evidence)["diagnostics"];
    assert_eq!(diagnostics["stage"], "decode");
    assert_eq!(diagnostics["path"], "choices");
    assert_eq!(diagnostics["response"]["id"], "partial-17");
    assert_eq!(diagnostics["response"]["usage"]["input_tokens"], 12);
    assert_eq!(diagnostics["response"]["error"]["code"], "invalid_shape");
    assert!(
        !serde_json::to_string(&evidence)
            .unwrap()
            .contains("private prompt")
    );
}

#[test]
fn oversized_metadata_retains_explicit_truncation_and_unknown_counts() {
    let evidence = metadata(
        &json!({"request_id":"large-17","usage":{"output_tokens":null},
        "timings":vec![3;100]}),
        &[],
    );
    assert_eq!(tree(&evidence)["usage"]["output_tokens"], Value::Null);
    assert_eq!(tree(&evidence)["timings"][32]["truncated_items"], 68);
}
