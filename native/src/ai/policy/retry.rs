//! Retain server-reported retry diagnostics.
use crate::model::AppError;
use serde_json::{Value, json};

pub(crate) fn annotate(error: &mut AppError, history: &[Value]) {
    error.message = format!(
        "{} Automatic rate-limit retries scheduled: {}.",
        error
            .message
            .replace(" No automatic retry was made.", "")
            .replace(" no automatic retry was made.", ""),
        history.len()
    );
    error.diagnostics.get_or_insert_with(|| json!({}))["automatic_retries"] = json!(history);
}
