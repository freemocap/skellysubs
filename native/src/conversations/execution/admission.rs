use super::*;

// Bounds include running, paused and dependency-waiting operations.
pub(super) const OUTSTANDING_NETWORK_LIMIT: i64 = 512;
pub(super) const TURN_ATTEMPT_LIMIT: i64 = 16;

pub(super) fn budget_error(message: &str) -> AppError {
    crate::diagnostics::native_event(
        "work_budget_rejected",
        &[
            ("outstanding_limit", OUTSTANDING_NETWORK_LIMIT as u64),
            ("turn_attempt_limit", TURN_ATTEMPT_LIMIT as u64),
        ],
    );
    AppError::new(ErrorCode::AdmissionHeld, message)
}

pub(crate) fn admit_network_work(
    db: &Connection,
    runtime: &graph_runtime::Runtime,
    additional: i64,
) -> Result<()> {
    let outstanding = graph_runtime::outstanding(db, runtime)?;
    if additional < 0 || additional > OUTSTANDING_NETWORK_LIMIT - outstanding {
        return Err(budget_error(
            "AI work queue is full. Let pending work finish or cancel it before submitting again. This action was not accepted.",
        ));
    }
    Ok(())
}
