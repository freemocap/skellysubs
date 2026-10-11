//! Shared recognizer execution. Recording inspection and product publication remain with owners.
use super::*;
use crate::ai::{
    audio::TranscriptionRequest,
    results::{self, Retained},
};

impl Application {
    pub(super) fn transcription_authority(
        &self,
        target: &access::ResolvedTarget,
        install: &str,
    ) -> Result<()> {
        let store = self.lock()?;
        if store.snapshot()?.learner.id != install {
            return Err(AppError::new(
                ErrorCode::SessionExpired,
                "Workspace changed during transcription.",
            ));
        }
        crate::ai::connections::speech_routing::validate_access(
            &store.connection,
            access::Capability::Transcription,
            target,
        )?;
        if crate::ai::connections::configuration::config(&store.connection)?.paused {
            return Err(AppError::new(
                ErrorCode::AdmissionHeld,
                "Transcription stopped: AI execution is paused.",
            ));
        }
        Ok(())
    }

    pub(crate) async fn shared_transcription(
        self: &Arc<Self>,
        target: access::ResolvedTarget,
        input: TranscriptionRequest,
        install: String,
        consumer: &str,
        validate_consumer: impl Fn() -> Result<()>,
    ) -> Result<Retained> {
        validate_consumer()?;
        if input.wav.is_empty() || input.wav.len() > 25 * 1024 * 1024 {
            return Err(AppError::new(
                ErrorCode::Validation,
                "Recording must contain audio and be at most 25 MB.",
            ));
        }
        crate::ai::content::save_recording(&self.lock()?.connection, consumer, &input.wav)?;
        audio::validate_transcription_language(&target, &input.language)?;
        let key = results::transcription::request_key(&target, &input, &install)?;
        let (subscription, producer) = self.transcription_pending.subscribe(key.clone())?;
        if let Some(producer) = producer {
            let producer = Arc::new(producer);
            let prepared = super::native_transcription::begin(
                self,
                target,
                input,
                install.clone(),
                key,
                &producer,
            )?;
            let state = self.clone();
            tauri::async_runtime::spawn(async move {
                let result =
                    super::native_transcription::execute(&state, &install, &producer, prepared)
                        .await;
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
