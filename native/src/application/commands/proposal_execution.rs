//! One structured-generation executor for persona and Drill proposals.
use super::*;
mod native;

pub(super) struct Output<T> {
    pub completion: Option<provider::Completion>,
    pub outcome: Result<T>,
}
pub(super) struct Task {
    pub messages: Vec<provider::PromptMessage>,
    pub length: crate::drill::generation::DrillLength,
    pub max_output_tokens: i32,
}

struct ConsumerGuard {
    state: Arc<Application>,
    request: Arc<generation::Request>,
    received: bool,
}
impl Drop for ConsumerGuard {
    fn drop(&mut self) {
        if self.received {
            return;
        }
        self.request.cancel();
        let settled = (|| {
            let mut store = self.state.lock()?;
            if store.learner_id()? != self.request.install_id {
                return Ok(());
            }
            generation_receipts::cancel(&mut store, &self.request)
        })();
        if let Err(error) = settled {
            crate::diagnostics::failures::report("proposal_consumer_cancellation", &error);
        }
    }
}

pub(super) async fn execute<T: Send + 'static>(
    state: &Arc<Application>,
    request: Arc<generation::Request>,
    task: Task,
    parse: impl FnOnce(&Store, &generation::Request, &provider::Completion) -> Result<T>
    + Send
    + 'static,
) -> Output<T> {
    let mut consumer = ConsumerGuard {
        state: state.clone(),
        request: request.clone(),
        received: false,
    };
    let state = state.clone();
    let (sender, receiver) = tokio::sync::oneshot::channel();
    // A proposal is intentionally fresh: no cache lookup and no prompt deduplication.
    // The detached worker owns settlement if its command future disappears.
    tokio::spawn(async move {
        let output = native::produce(&state, &request, task, parse, || !sender.is_closed()).await;
        if let Err(output) = sender.send(output) {
            let outcome: Result<()> = Err(AppError::new(
                if request.was_submitted() {
                    ErrorCode::UnknownOutcome
                } else {
                    ErrorCode::Conflict
                },
                "Proposal consumer closed. Its result was not adopted.",
            ));
            let settled = (|| {
                let mut store = state.lock()?;
                if store.learner_id()? != request.install_id {
                    return Ok(());
                }
                generation_receipts::finish(
                    &mut store,
                    &request,
                    output.completion.as_ref(),
                    &outcome,
                )
            })();
            if let Err(error) = settled {
                crate::diagnostics::failures::report("proposal_consumer_settlement", &error);
            }
        }
    });
    let output = receiver.await.unwrap_or_else(|_| Output {
        completion: None,
        outcome: Err(AppError::new(
            ErrorCode::UnknownOutcome,
            "Proposal execution ended before reporting its outcome.",
        )),
    });
    consumer.received = true;
    output
}
