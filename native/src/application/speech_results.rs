//! Composition of shared speech execution and workspace storage. No product ownership.
use super::*;
#[cfg(test)]
use crate::ai::results;
use crate::ai::results::Retained;
#[cfg(test)]
use serde_json::json;

use crate::ai::results::speech::{request_key, scope};

impl Application {
    pub(super) fn speech_authority(
        &self,
        target: &access::ResolvedTarget,
        install: &str,
    ) -> Result<()> {
        let store = self.lock()?;
        let current_install: String =
            store
                .connection
                .query_row("SELECT id FROM learner LIMIT 1", [], |r| r.get(0))?;
        if current_install != install {
            return Err(AppError::new(
                ErrorCode::SessionExpired,
                "Workspace changed during speech execution.",
            ));
        }
        crate::ai::connections::speech_routing::validate_access(
            &store.connection,
            access::Capability::Speech,
            target,
        )?;
        if crate::ai::connections::configuration::config(&store.connection)?.paused {
            return Err(AppError::new(
                ErrorCode::Conflict,
                "Speech access changed before execution.",
            ));
        }
        Ok(())
    }

    pub(super) async fn shared_speech(
        self: &Arc<Self>,
        target: access::ResolvedTarget,
        input: audio::SpeechInput,
        install: String,
        consumer: &str,
        validate_consumer: impl Fn() -> Result<()>,
    ) -> Result<Retained> {
        validate_consumer()?;
        audio::validate_speech(&target, &input)?;
        let service_scope = scope(&target, &install)?;
        // Pending work shares the same local identity as retained results.
        let pending_key = request_key(&service_scope, &input)?;
        let (subscription, producer) = self.speech_pending.subscribe(pending_key)?;
        if let Some(producer) = producer {
            let producer = Arc::new(producer);
            let prepared = super::native_speech::begin(self, &target, &input, &install, &producer)?;
            let state = self.clone();
            tauri::async_runtime::spawn(async move {
                let result =
                    super::native_speech::execute(&state, &install, &producer, prepared).await;
                if let Ok(producer) = Arc::try_unwrap(producer) {
                    producer.finish(result);
                }
            });
        }
        super::native_speech::associate(&self.lock()?.connection, consumer, subscription.id())?;
        let result = subscription.wait();
        tokio::pin!(result);
        loop {
            tokio::select! {
                result = &mut result => {
                    validate_consumer()?;
                    return result;
                },
                _ = tokio::time::sleep(Duration::from_millis(50)) => validate_consumer()?,
            }
        }
    }
}

#[cfg(test)]
#[path = "tests/shared_speech.rs"]
mod tests;
