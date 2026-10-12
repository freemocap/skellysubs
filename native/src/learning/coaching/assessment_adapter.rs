//! Combined, source-bound Jev assessment and deterministic skill-credit projection.
use crate::ai::transport::provider::Completion;
use crate::learning::practice_assessment::{Instructions, SkillPrompt};
use crate::model::{AppError, AssessmentAdapter, ErrorCode, Result};
use serde_json::{Value, json};
use std::collections::BTreeSet;

pub const MODEL: &str = "typesafe/jev-1.13";
pub const VERSION: &str = "jev-turn-assessment-1";
pub fn version(_: AssessmentAdapter) -> &'static str {
    VERSION
}
fn fail(message: &str) -> AppError {
    AppError::new(ErrorCode::Validation, message)
}

/// Pure result validation and credit eligibility projection. Publication and
/// source authority remain the owner's transaction; no current settings lookup.
pub fn validate_captured(
    output: &Completion,
    skills: &[SkillPrompt],
    config: &Instructions,
) -> Result<Value> {
    if output.finish_reason == "error" || output.text.len() > 100000 {
        return Err(fail("Skill presence requires a bounded Jev completion."));
    }
    let ids: BTreeSet<_> = skills.iter().map(|s| s.id.clone()).collect();
    let raw: Value = serde_json::from_str(&output.text).map_err(|cause| {
        crate::diagnostics::response::json_context(
            &cause,
            "skill_presence",
            fail("Invalid skill presence JSON."),
        )
    })?;
    let assessment = crate::learning::turn_assessment::validate(&raw, &ids)?;
    let answers = &assessment.skills;
    config.attribution.validate()?;
    let presence = answers
        .iter()
        .map(|(id, answer)| {
            let selected = config.attribution.accepts(answer);
            (
                id,
                if selected
                    || matches!(
                        answer.choice,
                        crate::learning::practice::Presence::Absent
                            | crate::learning::practice::Presence::Unclear
                    )
                {
                    answer.choice
                } else {
                    crate::learning::practice::Presence::Absent
                },
            )
        })
        .collect::<std::collections::BTreeMap<_, _>>();
    Ok(
        json!({"presence":presence,"answers":answers,"grammar":assessment.grammar,"understandability":assessment.understandability,"model":output.actual_model,"adapter":"jev_choice","promptVersion":VERSION,"policy":{"version":crate::learning::practice::POLICY,"minimumPositiveProbability":config.attribution.minimum_positive_probability}}),
    )
}
