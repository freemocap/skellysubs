//! Source-bound phrase attribution; never awards or changes skill credit.
use crate::ai::transport::provider::{Completion, PromptMessage};
use crate::learning::{practice::Presence, practice_assessment::Answer};
use crate::model::{AppError, ErrorCode, Result};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::collections::{BTreeMap, BTreeSet};
use unicode_segmentation::UnicodeSegmentation;

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Config {
    pub minimum_positive_probability: f64,
    pub instructions: String,
}
impl Config {
    pub fn validate(&self) -> Result<()> {
        if !self.minimum_positive_probability.is_finite()
            || !(0.0..=1.0).contains(&self.minimum_positive_probability)
            || self.instructions.trim().is_empty()
        {
            return Err(fail(
                "configuration",
                "finite threshold in [0,1] and nonempty instructions",
            ));
        }
        Ok(())
    }
    pub fn accepts(&self, answer: &Answer) -> bool {
        matches!(answer.choice, Presence::Direct | Presence::Contextual)
            && answer.probabilities["direct"] + answer.probabilities["contextual"]
                >= self.minimum_positive_probability
    }
}
fn fail(path: &str, expected: &str) -> AppError {
    AppError::new(
        ErrorCode::Validation,
        "Skill attribution could not be validated.",
    )
    .with_diagnostics(
        json!({"validation":{"stage":"skill_attribution","path":path,"expected":expected}}),
    )
}
pub fn selected(captured: &Value) -> Result<BTreeSet<String>> {
    let presence: BTreeMap<String, Presence> =
        serde_json::from_value(captured["skillAssessment"]["presence"].clone())?;
    Ok(presence
        .into_iter()
        .filter_map(|(id, p)| matches!(p, Presence::Direct | Presence::Contextual).then_some(id))
        .collect())
}
pub fn schema(ids: &BTreeSet<String>) -> Value {
    json!({"type":"object","additionalProperties":false,"required":["skills"],"properties":{"skills":{"type":"array","minItems":ids.len(),"maxItems":ids.len(),"items":{"type":"object","additionalProperties":false,"required":["skill_id","spans"],"properties":{"skill_id":{"type":"string","enum":ids},"spans":{"type":"array","maxItems":8,"items":{"type":"object","additionalProperties":false,"required":["quote","occurrence"],"properties":{"quote":{"type":"string","minLength":1,"maxLength":4096},"occurrence":{"type":"integer","minimum":0,"maximum":4096}}}}}}}}})
}

pub(crate) fn prompt_for_selection(
    source: &str,
    selected: &BTreeSet<String>,
    catalog: &[crate::learning::practice_assessment::SkillPrompt],
    config: &Config,
) -> Result<Vec<PromptMessage>> {
    let skills: Vec<_> = catalog
        .iter()
        .filter(|s| selected.contains(&s.id))
        .collect();
    if skills.len() != selected.len() || skills.is_empty() {
        return Err(fail("skills", "selected skills in captured catalog"));
    }
    config.validate()?;
    let data = json!({"learnerMessage":source,"skills":skills});
    if data.to_string().len() + config.instructions.len() > 96000 {
        return Err(fail("input", "at most 96 KB"));
    }
    Ok(vec![
        PromptMessage {
            role: "system".into(),
            content: config.instructions.clone(),
        },
        PromptMessage {
            role: "user".into(),
            content: data.to_string(),
        },
    ])
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Response {
    skills: Vec<Item>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Item {
    skill_id: String,
    spans: Vec<Quote>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Quote {
    quote: String,
    occurrence: usize,
}

/// Offsets are UTF-16 for display consumers; matching and boundaries use original UTF-8.
fn locate(source: &str, quote: &Quote) -> Option<Value> {
    if quote.quote.trim().is_empty() {
        return None;
    }
    let start = source
        .char_indices()
        .map(|(i, _)| i)
        .filter(|i| source[*i..].starts_with(&quote.quote))
        .nth(quote.occurrence)?;
    let end = start + quote.quote.len();
    let boundaries: BTreeSet<_> = source
        .grapheme_indices(true)
        .map(|(i, _)| i)
        .chain([source.len()])
        .collect();
    if !boundaries.contains(&start) || !boundaries.contains(&end) {
        return None;
    }
    Some(
        json!({"quote":quote.quote,"start":source[..start].encode_utf16().count(),"end":source[..end].encode_utf16().count()}),
    )
}
pub(crate) fn validate_source(
    source: &str,
    ids: &BTreeSet<String>,
    output: &Completion,
) -> Result<Value> {
    if output.finish_reason == "error" || output.text.len() > 100000 {
        return Err(fail("response", "bounded completed response"));
    }
    let response: Response = crate::diagnostics::structured::decode(
        &output.text,
        &schema(ids),
        "Skill attribution rejected",
    )?;
    let mut skills = BTreeMap::new();
    for item in response.skills {
        if !ids.contains(&item.skill_id) || skills.contains_key(&item.skill_id) {
            return Err(fail("skills", "exactly one entry per selected skill"));
        }
        let spans: Option<Vec<_>> = item.spans.iter().map(|q| locate(source, q)).collect();
        let reason = if item.spans.is_empty() {
            "not_localized"
        } else if spans.is_none() {
            "unmatched_quote"
        } else {
            "localized"
        };
        let spans = if reason == "localized" {
            spans.unwrap()
        } else {
            vec![]
        };
        skills.insert(item.skill_id, json!({"evidence_kind":if spans.is_empty(){"whole_message"}else{"quoted"},"spans":spans,"reason":reason}));
    }
    if skills.keys().cloned().collect::<BTreeSet<_>>() != *ids {
        return Err(fail("coverage", "all selected skills"));
    }
    Ok(json!({"skills":skills,"model":output.actual_model}))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn original_unicode_and_occurrence_are_required() {
        let q = |text: &str, occurrence| Quote {
            quote: text.into(),
            occurrence,
        };
        assert_eq!(locate("🙂 ayer ayer", &q("ayer", 1)).unwrap()["start"], 8);
        assert!(locate("e\u{301}", &q("é", 0)).is_none());
        assert!(locate("e\u{301}", &q("e", 0)).is_none());
        assert!(locate("العربية", &q("العربية", 0)).is_some());
        assert!(locate("中文", &q("中文", 0)).is_some());
        assert!(locate("हिन्दी", &q("हिन्दी", 0)).is_some());
        assert_eq!(locate("aaaa", &q("aa", 1)).unwrap()["start"], 1);
    }
    #[test]
    fn threshold_requires_positive_category_and_combined_probability() {
        let config = Config {
            minimum_positive_probability: 0.6,
            instructions: "Locate".into(),
        };
        let mut answer = Answer {
            choice: Presence::Direct,
            confidence: 0.4,
            probabilities: BTreeMap::from([("direct".into(), 0.4), ("contextual".into(), 0.2)]),
        };
        assert!(config.accepts(&answer));
        answer.probabilities.insert("contextual".into(), 0.1);
        assert!(!config.accepts(&answer));
        answer.choice = Presence::Unclear;
        answer.probabilities.insert("contextual".into(), 0.5);
        assert!(!config.accepts(&answer));
        assert!(
            Config {
                minimum_positive_probability: f64::NAN,
                instructions: "x".into()
            }
            .validate()
            .is_err()
        );
    }
}
