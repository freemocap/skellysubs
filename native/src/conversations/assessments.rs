//! Immutable assessment results belong to a message version and one execution attempt.
//! The caller owns the publication transaction. Turn JSON holds execution inputs and
//! derived learning projections; correction observations are read from this repository.
use crate::model::*;
use rusqlite::{Connection, OptionalExtension, params};
use serde_json::Value;
mod native;

/// Message-level judgments from the current, source-owned assessment. Numeric
/// evidence remains readable for saved attempts that used that result shape.
pub(crate) fn feedback(
    db: &Connection,
    turn: &str,
) -> Result<Option<crate::learning::coaching::conversation_support::ConversationFeedback>> {
    feedback_using(db, turn, &mut |kind| current(db, turn, kind))
}

fn feedback_using(
    db: &Connection,
    turn: &str,
    current: &mut impl FnMut(&str) -> Result<Option<(String, Value)>>,
) -> Result<Option<crate::learning::coaching::conversation_support::ConversationFeedback>> {
    use crate::learning::coaching::conversation_support::ConversationFeedback;
    if let Some((_, result)) = current("skill_assessment")?
        && (result.get("grammar").is_some() || result.get("understandability").is_some())
    {
        let answers = ["grammar", "understandability"]
            .into_iter()
            .map(|key| Ok((key.to_owned(), serde_json::from_value(result[key].clone())?)))
            .collect::<Result<_>>()?;
        return Ok(Some(ConversationFeedback {
            grammar: None,
            conversation: None,
            answers,
        }));
    }
    // A captured combined assessment owns this view even while pending/failed.
    let combined: bool = db.query_row(
        "SELECT json_type(context,'$.messageAssessmentQuestions') IS NOT NULL FROM turns WHERE id=?1",
        [turn], |r| r.get(0),
    )?;
    if combined {
        return Ok(None);
    }
    current("conversation_feedback")?
        .map(|(_, result)| Ok(serde_json::from_value(result)?))
        .transpose()
}

/// A pending/failed reassessment does not expose an older success as current.
pub(crate) fn current(db: &Connection, turn: &str, kind: &str) -> Result<Option<(String, Value)>> {
    native::current(db, turn, kind)
}

/// Build the policy input from source-owned results. Disclosure belongs to the
/// producing attempt; opening an old card cannot disclose a new assessment.
pub(crate) fn context(db: &Connection, turn: &str) -> Result<Value> {
    context_using(db, turn, &mut |kind| current(db, turn, kind))
}

/// Product reads use the running engine's verified records rather than replaying
/// all history for every displayed assessment. Historical-only owners still use
/// the same recorded inspection path as exports and disclosure commands.
pub(crate) fn view(
    db: &Connection,
    turn: &str,
    runtime: &crate::conversations::execution::graph_runtime::Runtime,
) -> Result<(
    Value,
    Option<crate::learning::coaching::conversation_support::ConversationFeedback>,
)> {
    if db.is_autocommit() {
        let tx = db.unchecked_transaction()?;
        return view(&tx, turn, runtime);
    }
    let mut read = |kind: &str| native::current_using(db, turn, kind, Some(runtime));
    Ok((
        context_using(db, turn, &mut read)?,
        feedback_using(db, turn, &mut read)?,
    ))
}

fn context_using(
    db: &Connection,
    turn: &str,
    current: &mut impl FnMut(&str) -> Result<Option<(String, Value)>>,
) -> Result<Value> {
    let raw: String = db.query_row("SELECT context FROM turns WHERE id=?1", [turn], |r| {
        r.get(0)
    })?;
    let mut context: Value = serde_json::from_str(&raw)?;
    if let Some((attempt, result)) = current("coach_feedback")? {
        context["coachObservation"] = result["observation"].clone();
        let disclosed = native::disclosure(db, &attempt)?;
        context["coachDecision"] = match disclosed {
            Some(raw) => serde_json::from_str(&raw)?,
            None => result["decision"].clone(),
        };
        context["coachObservationAttempt"] = Value::String(attempt);
        context["coachValidationOmissions"] = result["validationOmissions"].clone();
    } else {
        for field in [
            "coachObservation",
            "coachDecision",
            "coachObservationAttempt",
            "coachValidationOmissions",
        ] {
            context
                .as_object_mut()
                .ok_or_else(|| AppError::new(ErrorCode::Storage, "Invalid turn context."))?
                .remove(field);
        }
    }
    Ok(context)
}

/// The policy selected the current receipt immediately before this call in the
/// command transaction. Disclosure retains its producing assessment identity.
pub(crate) fn disclose(db: &Connection, turn: &str, receipt: &str, decision: &Value) -> Result<()> {
    if !native::current(db, turn, "coach_feedback")?.is_some_and(|(id, _)| id == receipt) {
        return Err(AppError::new(
            ErrorCode::Conflict,
            "The feedback disclosure source is no longer current.",
        ));
    }
    native::disclose(db, receipt, decision)
}
