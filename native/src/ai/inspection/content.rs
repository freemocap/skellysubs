//! Graph work never contains resolved secrets. Hide credential-store references
//! at declared request/configuration boundaries without redacting source text.
use crate::ai::graph::Values;
use serde_json::{Value, json};

pub(super) fn inputs(mut values: Values) -> Value {
    for (port, value) in &mut values {
        if port == "credential" || port.ends_with("_credential") {
            *value = json!({"omitted":"credential reference"});
        } else if port == "target" || port.ends_with("_target") {
            target(value);
        } else if port == "request"
            && let Some(target_value) = value.get_mut("target")
        {
            target(target_value);
        }
    }
    json!(values)
}
fn target(value: &mut Value) {
    if let Some(credential) = value.get_mut("credential") {
        *credential = json!({"omitted":"credential reference"});
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn credential_references_are_separate_from_unchanged_source_content() {
        let source = json!({"credential":"quoted source", "text":"raw café 𠮷\n"});
        let result = inputs(Values::from([
            ("credential".into(), json!("key-store-reference")),
            ("context".into(), source.clone()),
            (
                "request".into(),
                json!({"target":{"credential":"secret-reference"},"messages":[{"content":"exact prompt"}]}),
            ),
        ]));
        assert_eq!(result["context"], source);
        assert_eq!(result["request"]["messages"][0]["content"], "exact prompt");
        assert!(!result.to_string().contains("secret-reference"));
        assert!(!result.to_string().contains("key-store-reference"));
    }
}
