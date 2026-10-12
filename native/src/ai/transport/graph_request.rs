//! Executes an already-authorized native producer through the existing transport.
//! The host owns permits, secret/protocol awaits, current-authority rechecks and
//! committed wire identities. This module neither creates IDs nor commits state.
use super::{graph_evidence, grouped, provider, text_request::TextRequest};
use crate::{
    ai::graph::{Fault, InvocationContext},
    model::Result,
};
use std::sync::Mutex;

/// An item can finish before the enclosing stream fails. Keep both facts, plus
/// capture failures, so the host never loses the original response to a core Fault.
pub struct Outcome {
    pub item: Option<Result<provider::Completion>>,
    pub transport: Result<()>,
    pub capture_failure: Option<Fault>,
}

/// `deltas` is the result of the host's protocol negotiation. Call only after
/// its asynchronous boundaries have been followed by native/domain revalidation.
/// Output mode is explicit: structured branches retain their schema and token
/// limits through the shared transport. Domain result validation remains the
/// operation's responsibility; this adapter retains response evidence unchanged.
pub async fn execute(
    client: &reqwest::Client,
    key: &str,
    request: &TextRequest,
    output: provider::RequestOutput<'_>,
    invocation: &InvocationContext,
    deltas: bool,
) -> Outcome {
    let mut item = None;
    let capture_failure = Mutex::new(None);
    let preview = Mutex::new(String::new());
    let capture = |result: std::result::Result<(), Fault>| {
        if let Err(error) = result {
            capture_failure
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .get_or_insert(error);
        }
    };
    let mut private: Vec<&str> = request
        .messages
        .iter()
        .map(|message| message.content.as_str())
        .collect();
    private.push(key);
    let transport = grouped::request_streaming(
        client,
        key,
        std::slice::from_ref(request),
        &[output],
        deltas,
        |_, result| {
            // Observe before EOF, retaining a validated item even if another
            // frame or the enclosing completion marker subsequently fails.
            let preview = preview.lock().unwrap_or_else(|e| e.into_inner());
            let mut private = private.clone();
            private.push(&preview);
            let evidence = match &result {
                Ok(completion) => graph_evidence::completion(completion, &request.model, &private),
                Err(error) => graph_evidence::failure(error, &request.model, &private),
            };
            capture(invocation.observe(evidence));
            item = Some(result);
            Ok(())
        },
        |_, text| {
            *preview.lock().unwrap_or_else(|e| e.into_inner()) = text.into();
            capture(invocation.provisional_text(text));
        },
    )
    .await;
    if let Err(error) = &transport {
        let preview = preview.lock().unwrap_or_else(|e| e.into_inner());
        private.push(&preview);
        if let Some(Ok(completion)) = &item {
            private.push(&completion.text);
        }
        capture(invocation.observe(graph_evidence::failure(error, &request.model, &private)));
    }
    Outcome {
        item,
        transport,
        capture_failure: capture_failure
            .into_inner()
            .unwrap_or_else(|e| e.into_inner()),
    }
}

#[cfg(test)]
mod tests;
