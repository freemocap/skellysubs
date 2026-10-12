//! Bounded one-time delivery of completed audio to asynchronous playback callers.
//! Reusable retention belongs to ai::results; reading this mailbox consumes it.
use crate::model::AppError;
use crate::model::ErrorCode;
use crate::model::Result;
use std::{cell::RefCell, collections::VecDeque};

pub const AUDIO_LIMIT: usize = 4 * 1024 * 1024;
pub const DELIVERY_ENTRIES: usize = 4;
pub const DELIVERY_BYTES: usize = 16 * 1024 * 1024;

#[derive(Debug, Clone)]
pub struct Source {
    pub language_tag: String,
    pub message_id: String,
    pub text: String,
    pub language: String,
    pub voice: String,
}

#[derive(Debug, Clone)]
pub struct ReadyAudio {
    pub operation_id: String,
    pub attempt_id: String,
    pub message_id: String,
    pub wav: Vec<u8>,
    pub alignment: Option<crate::speech::alignment::SpeechAlignment>,
}

#[derive(Default)]
pub struct DeliveryBuffer {
    entries: RefCell<VecDeque<ReadyAudio>>,
}
impl DeliveryBuffer {
    fn bytes(&self) -> usize {
        self.entries.borrow().iter().map(|a| a.wav.len()).sum()
    }
    pub fn insert(&mut self, audio: ReadyAudio) -> Result<()> {
        if audio.wav.is_empty() || audio.wav.len() > AUDIO_LIMIT {
            return Err(AppError::new(
                ErrorCode::Validation,
                "Speech audio exceeds its delivery limits.",
            ));
        }
        self.remove_operation(&audio.operation_id);
        while self.entries.borrow().len() >= DELIVERY_ENTRIES
            || self.bytes() + audio.wav.len() > DELIVERY_BYTES
        {
            self.entries.get_mut().pop_front();
        }
        self.entries.get_mut().push_back(audio);
        Ok(())
    }
    pub fn contains(&self, attempt_id: &str) -> bool {
        self.entries
            .borrow()
            .iter()
            .any(|a| a.attempt_id == attempt_id)
    }
    pub fn get(&self, attempt_id: &str) -> Option<ReadyAudio> {
        let mut entries = self.entries.borrow_mut();
        let index = entries.iter().position(|a| a.attempt_id == attempt_id)?;
        entries.remove(index)
    }
    pub fn peek(&self, attempt_id: &str) -> Option<ReadyAudio> {
        self.entries
            .borrow()
            .iter()
            .find(|audio| audio.attempt_id == attempt_id)
            .cloned()
    }
    pub fn discard(&self, attempt_id: &str) {
        self.get(attempt_id);
    }
    pub fn remove_operation(&mut self, operation_id: &str) {
        self.entries
            .get_mut()
            .retain(|a| a.operation_id != operation_id);
    }
}

impl crate::storage::store::Store {
    pub(crate) fn delivered_speech_owner(
        &self,
        operation: &str,
        attempt: &str,
        audio: &crate::speech::alignment::SpeechAudio,
    ) -> Result<crate::speech::recording::owner::RecordingOwner> {
        self.graph_runtime
            .delivered_speech_owner(&self.connection, operation, attempt, audio)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn audio(operation: &str, attempt: &str, size: usize) -> ReadyAudio {
        ReadyAudio {
            operation_id: operation.into(),
            attempt_id: attempt.into(),
            message_id: "source".into(),
            wav: vec![0; size],
            alignment: None,
        }
    }
    #[test]
    fn delivery_is_bounded_and_reads_do_not_restore_evicted_audio() {
        let mut cache = DeliveryBuffer::default();
        for index in 0..5 {
            cache
                .insert(audio(&index.to_string(), &index.to_string(), AUDIO_LIMIT))
                .unwrap();
        }
        assert_eq!(cache.entries.borrow().len(), 4);
        assert_eq!(cache.bytes(), DELIVERY_BYTES);
        assert!(cache.get("4").is_some());
        for _ in 0..20 {
            assert!(cache.get("0").is_none());
            assert!(cache.get("4").is_none());
        }
        assert_eq!(cache.bytes(), 3 * AUDIO_LIMIT);
        cache.remove_operation("4");
        assert!(cache.get("4").is_none());
        assert_eq!(cache.bytes(), 3 * AUDIO_LIMIT);
    }
    #[test]
    fn replacement_and_rejection_do_not_leak_or_destroy_previous_audio() {
        let mut cache = DeliveryBuffer::default();
        cache.insert(audio("op", "first", 16)).unwrap();
        assert!(cache.insert(audio("op", "bad", AUDIO_LIMIT + 1)).is_err());
        assert!(cache.insert(audio("op", "empty", 0)).is_err());
        assert!(cache.get("first").is_some());
        cache.insert(audio("op", "second", 32)).unwrap();
        assert!(cache.get("first").is_none());
        assert_eq!(cache.bytes(), 32);
        assert_eq!(cache.entries.borrow().len(), 1);
    }
}
