//! Deterministic disclosure. Hidden hypotheses stay in source-owned assessment results;
//! display decisions contain only the help the learner has chosen to see.
use crate::learning::coaching::*;
use crate::model::*;
use rusqlite::{Connection, OptionalExtension};
use serde::Deserialize;
use serde_json::{Value, json};
use std::collections::HashMap;

#[derive(Deserialize)]
struct Intensity {
    start_at: String,
    show_logged: bool,
}
#[derive(Deserialize)]
struct Policy {
    correct_only: String,
    skip_sources: Vec<String>,
    intensity: HashMap<String, Intensity>,
}
fn invalid(message: &str) -> AppError {
    AppError::new(ErrorCode::Validation, message)
}
fn movement(value: &str) -> Result<CoachMove> {
    serde_json::from_value(json!(value)).map_err(|cause| {
        crate::diagnostics::response::json_context(
            &cause,
            "coach_policy_decode",
            invalid("Unknown configured coach move."),
        )
    })
}
fn summary(item: &ObservedItem) -> ObservedItemSummary {
    ObservedItemSummary {
        construct: item.construct.clone(),
        quote: item.quote.clone(),
        outcome: item.outcome,
        rationale: if item.error.is_none() {
            item.rationale.clone()
        } else {
            String::new()
        },
    }
}
pub(crate) fn unchanged_correction(item: &ObservedItem) -> bool {
    item.error
        .as_ref()
        .is_some_and(|e| e.target_hypothesis.trim() == item.quote.trim())
}
fn actionable(item: &ObservedItem) -> bool {
    item.error.is_some()
        && !unchanged_correction(item)
        && matches!(item.outcome, Outcome::Partial | Outcome::NotDemonstrated)
}
pub(crate) fn view(captured: &Value) -> Result<Option<CoachObservationView>> {
    let Some(raw) = captured.get("coachObservation") else {
        return Ok(None);
    };
    let observation: CoachObservation = serde_json::from_value(raw.clone())?;
    let mut corrections = Vec::new();
    if captured["coachDecision"]["exposedMove"] == "explicit"
        && captured["coachDecision"]["keptGoing"] != true
    {
        for item in observation.items.iter().filter(|item| actionable(item)) {
            let value = correction(item, CoachMove::Explicit, captured)?;
            if !corrections
                .iter()
                .any(|c: &Correction| c.quote == value.quote && c.text == value.text)
            {
                corrections.push(value);
            }
        }
    }
    let omitted = observation
        .items
        .iter()
        .filter(|item| unchanged_correction(item))
        .count();
    let validation_omissions = captured["coachValidationOmissions"].as_u64().unwrap_or(0);
    Ok(Some(CoachObservationView {
        issues: observation
            .items
            .iter()
            .filter(|item| actionable(item))
            .map(|item| CoachIssue {
                quote: item.quote.clone(),
                severity: if item.outcome == Outcome::Partial {
                    CoachIssueSeverity::Partial
                } else {
                    CoachIssueSeverity::Error
                },
            })
            .collect(),
        corrections,
        notes: if validation_omissions > 0 {
            vec![format!(
                "{validation_omissions} unusable assessment item(s) omitted during validation; a clean verdict is unavailable."
            )]
        } else if omitted == 0 {
            vec![]
        } else {
            vec![format!(
                "{omitted} unchanged replacement(s) omitted from correction display. Other feedback is retained."
            )]
        },
        meaning_recovered: observation.meaning_recovered,
        items: observation.items.iter().map(summary).collect(),
        candidates_sent: captured["candidateConstructs"]
            .as_array()
            .ok_or_else(|| invalid("Missing captured candidates."))?
            .len(),
        items_returned: observation.items.len(),
    }))
}
fn correction(item: &ObservedItem, rung: CoachMove, _captured: &Value) -> Result<Correction> {
    let error = item
        .error
        .as_ref()
        .ok_or_else(|| invalid("Correction requires an observed error."))?;
    let text = match rung {
        CoachMove::Hint => error.hint.clone(),
        CoachMove::Elicit | CoachMove::PartnerClarify => error.elicitation.clone(),
        CoachMove::Metalinguistic => error.metalinguistic.clone(),
        CoachMove::Explicit => error.target_hypothesis.clone(),
    };
    Ok(Correction {
        construct: item.construct.clone(),
        quote: item.quote.clone(),
        r#move: rung.clone(),
        explanation: (rung == CoachMove::Explicit).then(|| item.rationale.clone()),
        text,
    })
}
/// The one help mode requested from the model. Other cue fields stay empty.
pub(crate) fn requested_move(captured: &Value) -> Result<CoachMove> {
    let policy: Policy = serde_json::from_value(captured["feedbackPolicy"].clone())?;
    let intensity = match captured["practiceSettings"]["coachProactivity"].as_str() {
        Some("on_request") => "light",
        Some("occasional") => "standard",
        Some("frequent") => "thorough",
        _ => return Err(invalid("Unknown coach intensity.")),
    };
    let intensity = policy
        .intensity
        .get(intensity)
        .ok_or_else(|| invalid("Missing feedback intensity policy."))?;
    movement(&intensity.start_at)
}

pub(crate) fn decide(captured: &Value, observation: &CoachObservation) -> Result<CoachDecision> {
    let policy: Policy = serde_json::from_value(captured["feedbackPolicy"].clone())?;
    let intensity = match captured["practiceSettings"]["coachProactivity"].as_str() {
        Some("on_request") => "light",
        Some("occasional") => "standard",
        Some("frequent") => "thorough",
        _ => return Err(invalid("Unknown coach intensity.")),
    };
    let intensity = policy
        .intensity
        .get(intensity)
        .ok_or_else(|| invalid("Missing feedback intensity policy."))?;
    let focus = captured["practiceFocus"]["id"].as_str();
    let mut decision = CoachDecision {
        exposed_move: None,
        shown: None,
        retry_invited: false,
        also_noticed: vec![],
        kept_going: false,
    };
    let selected = observation
        .items
        .iter()
        .filter(|i| {
            actionable(i)
                && i.error.as_ref().is_some_and(|e| {
                    let source = serde_json::to_value(&e.source).expect("enum serializes");
                    !policy.skip_sources.iter().any(|s| source == *s)
                        && (policy.correct_only == "useful_language"
                            || e.blocks_meaning
                            || focus == Some(i.construct.as_str()))
                })
        })
        .min_by_key(|i| !i.error.as_ref().unwrap().blocks_meaning);
    if let Some(item) = selected {
        let rung = requested_move(captured)?;
        decision.retry_invited = rung != CoachMove::Explicit;
        decision.shown = Some(correction(item, rung, captured)?);
    }
    if intensity.show_logged {
        decision.also_noticed = observation
            .items
            .iter()
            .filter(|i| {
                decision
                    .shown
                    .as_ref()
                    .is_none_or(|s| s.construct != i.construct)
            })
            .map(summary)
            .collect();
    }
    Ok(decision)
}
/// Applies a learner's coaching control to the turn's current decision. Other
/// work finishing meanwhile (glosses, assessments, rewards) does not matter: the
/// control reads the decision as it is now.
pub(crate) fn control(db: &Connection, turn: &str, control: CoachControl) -> Result<String> {
    let raw:Option<String>=db.query_row("SELECT context FROM turns t WHERE id=?1 AND NOT EXISTS(SELECT 1 FROM turns child WHERE child.replaces_turn_id=t.id)",[turn],|r|r.get(0)).optional()?;
    raw.ok_or_else(|| AppError::new(ErrorCode::NotFound, "Current coaching turn is unavailable."))?;
    let context = crate::conversations::assessments::context(db, turn)?;
    let mut decision: CoachDecision = serde_json::from_value(context["coachDecision"].clone())
        .map_err(|cause| {
            crate::diagnostics::response::json_context(
                &cause,
                "coach_policy_decode",
                invalid("No coaching decision is available."),
            )
        })?;
    match control {
        CoachControl::OpenCard => {
            if decision.kept_going {
                return Err(invalid("This correction was skipped."));
            }
            // A learner explicitly asking for analysis may inspect an error
            // that automatic interruption policy deliberately left unselected.
            if decision.shown.is_none() {
                let observation: CoachObservation =
                    serde_json::from_value(context["coachObservation"].clone())?;
                if let Some(item) = observation.items.iter().find(|item| actionable(item)) {
                    decision.shown = Some(correction(item, requested_move(&context)?, &context)?);
                    decision.retry_invited = requested_move(&context)? != CoachMove::Explicit;
                }
            }
            decision.exposed_move = decision.shown.as_ref().map(|shown| shown.r#move.clone());
        }

        CoachControl::KeepGoing => {
            decision.shown = None;
            decision.retry_invited = false;
            decision.kept_going = true;
        }
        CoachControl::ShowAnswer => {
            let selected = decision
                .shown
                .as_ref()
                .ok_or_else(|| invalid("No correction is selected."))?;
            let observation: CoachObservation =
                serde_json::from_value(context["coachObservation"].clone())?;
            let item = observation
                .items
                .iter()
                .find(|i| i.construct == selected.construct)
                .ok_or_else(|| invalid("Selected correction is unavailable."))?;
            decision.shown = Some(correction(item, CoachMove::Explicit, &context)?);
            decision.exposed_move = Some(CoachMove::Explicit);
            decision.retry_invited = false;
        }
    }
    let attempt = context["coachObservationAttempt"].clone();
    let decision = serde_json::to_value(decision)?;
    crate::conversations::assessments::disclose(
        db,
        turn,
        attempt
            .as_str()
            .ok_or_else(|| invalid("Missing assessment receipt."))?,
        &decision,
    )?;
    Ok(turn.into())
}

#[cfg(test)]
mod issue_tests {
    use super::*;

    fn captured(items: Value) -> Value {
        json!({
            "candidateConstructs": [],
            "coachObservation": {"meaning_recovered": "full", "items": items},
            "coachDecision": {"exposedMove": null, "shown": null, "keptGoing": false}
        })
    }

    #[test]
    fn skill_outcomes_without_errors_never_become_correctable_issues() {
        for outcome in Outcome::ALL {
            let saved = captured(json!([{
                "construct": "ability_permission_necessity", "quote": "je vais le porter.",
                "outcome": outcome, "error": null, "rationale": ""
            }]));
            let projected = view(&saved).unwrap().unwrap();
            assert!(projected.issues.is_empty());
            assert!(projected.corrections.is_empty());
            assert_eq!(projected.items[0].outcome, outcome);
        }
    }

    #[test]
    fn issues_use_the_same_actionable_items_as_corrections_before_disclosure() {
        let item = |quote: &str, target: &str, outcome: Outcome| {
            json!({
                "construct": "past", "quote": quote, "outcome": outcome, "rationale": "Use the past form.",
                "error": {"op": "replace", "category": "grammar", "source": "unknown",
                    "blocks_meaning": false, "target_hypothesis": target,
                    "hint": "", "elicitation": "", "metalinguistic": ""}
            })
        };
        let mut saved = captured(json!([
            item("I goes", "I go", Outcome::NotDemonstrated),
            item("he go", "he goes", Outcome::Partial),
            item("correct", "correct", Outcome::Partial)
        ]));
        let hidden = view(&saved).unwrap().unwrap();
        assert!(hidden.corrections.is_empty());
        assert_eq!(hidden.issues.len(), 2);
        assert!(matches!(
            hidden.issues[0].severity,
            CoachIssueSeverity::Error
        ));
        assert!(matches!(
            hidden.issues[1].severity,
            CoachIssueSeverity::Partial
        ));
        saved["coachDecision"]["exposedMove"] = json!("explicit");
        let disclosed = view(&saved).unwrap().unwrap();
        assert_eq!(disclosed.corrections.len(), hidden.issues.len());
        for (issue, correction) in hidden.issues.iter().zip(&disclosed.corrections) {
            assert_eq!(issue.quote, correction.quote);
        }
    }
}
