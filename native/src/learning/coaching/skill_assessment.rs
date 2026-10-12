//! Captured learner message and bounded preceding exchange for Jev presence.
use crate::ai::transport::provider::PromptMessage;
use crate::model::*;
use serde_json::{Value, json};
fn fail(s: &str) -> AppError {
    AppError::new(ErrorCode::Validation, format!("Skill assessment: {s}"))
}

/// Pure projection shared with the graph-definition inspector.
pub(crate) fn prompt_for_source(source: String, captured: &Value) -> Result<Vec<PromptMessage>> {
    let history = captured["messages"]
        .as_array()
        .ok_or_else(|| fail("missing exchange"))?;
    let previous: Vec<_> = history
        .iter()
        .skip(1)
        .take(history.len().saturating_sub(2))
        .rev()
        .take(4)
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
        .collect();
    let system = super::assessment_adapter::VERSION.to_owned();
    let mut data = json!({"currentLearnerMessage":source,"precedingExchange":previous,"input":captured["input"],"language":captured["languageContext"]["target_name"],"variety":captured["languageContext"]["variety_name"]});
    while system.len() + data.to_string().len() > 16000 {
        let old = data["precedingExchange"]
            .as_array_mut()
            .expect("owned context");
        if old.is_empty() {
            return Err(fail("current source exceeds the 16 KB request budget"));
        }
        old.remove(0);
    }
    Ok(vec![
        PromptMessage {
            role: "system".into(),
            content: system,
        },
        PromptMessage {
            role: "user".into(),
            content: data.to_string(),
        },
    ])
}
