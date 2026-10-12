//! Product-specific reply acceptance before settlement; the graph retains the
//! provider evidence and exposes an explicit failed attempt for learner retry.
use super::*;

pub(super) fn validate(db: &Connection, turn: &str, report: &mut InvocationReport) -> Result<()> {
    let Some(text) = report
        .outcome
        .as_ref()
        .ok()
        .and_then(|values| values.get("text"))
        .and_then(serde_json::Value::as_str)
    else {
        return Ok(());
    };
    if let Err(cause) = crate::conversations::phrase_start::validate(db, turn, text) {
        if cause.code != ErrorCode::Provider {
            return Err(cause);
        }
        report.observations.push(ResponseEvidence {
            error_code: Some("exact_phrase_missing".into()),
            validation: Some(ValidationEvidence {
                stage: "phrase_opening_validation".into(),
                path: "text".into(),
                expected: "opening containing the unchanged source phrase".into(),
            }),
            ..Default::default()
        });
        report.outcome = Err(Fault {
            code: "exact_phrase_missing".into(),
            path: "text".into(),
        });
    }
    Ok(())
}
