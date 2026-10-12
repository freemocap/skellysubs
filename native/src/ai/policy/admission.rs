//! One native inference pool, independent of route, model and window.
use crate::model::AppError;
use crate::model::ErrorCode;
use crate::model::Result;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::{OwnedSemaphorePermit, Semaphore};

// Provisional policy, not a provider guarantee. Tune from actual queue waits.
pub const NETWORK_CAPACITY: usize = 32;
const AUDIO_WAITING_CAPACITY: usize = 1;
/// OS keychain reads cannot be cancelled after their blocking call starts. Keep
/// admission inside that call so dropping an IPC/provider await cannot free a
/// slot while native credential access is still running.
pub struct CredentialReads {
    slots: Arc<Semaphore>,
}

impl CredentialReads {
    pub fn new() -> Self {
        Self {
            slots: Arc::new(Semaphore::new(NETWORK_CAPACITY)),
        }
    }

    pub async fn read<T: Send + 'static>(
        &self,
        read: impl FnOnce() -> Result<T> + Send + 'static,
    ) -> Result<T> {
        let permit = self.slots.clone().try_acquire_owned().map_err(|_| AppError::new(
            ErrorCode::AdmissionHeld,
            "Credential access is already at capacity. Finish pending system keychain prompts before trying again.",
        ))?;
        tauri::async_runtime::spawn_blocking(move || {
            let _permit = permit;
            read()
        })
        .await
        .map_err(|cause| {
            crate::diagnostics::failures::join(
                &cause,
                "credential_read_worker",
                AppError::new(ErrorCode::Internal, "Credential access failed."),
            )
        })?
    }
}

pub struct Admission {
    network: Arc<Semaphore>,
    // Chat waits durably in SQLite. Audio is volatile and must not accumulate
    // an unbounded collection of recordings while waiting for network capacity.
    audio_waiters: Arc<Semaphore>,
}

impl Admission {
    pub fn new() -> Self {
        Self::with_capacity(NETWORK_CAPACITY)
    }

    fn with_capacity(capacity: usize) -> Self {
        assert!(capacity > 0);
        Self {
            network: Arc::new(Semaphore::new(capacity)),
            audio_waiters: Arc::new(Semaphore::new(AUDIO_WAITING_CAPACITY)),
        }
    }

    pub fn try_chat(&self) -> Option<OwnedSemaphorePermit> {
        self.network.clone().try_acquire_owned().ok()
    }

    pub async fn audio(
        &self,
        mut validate: impl FnMut() -> Result<()>,
    ) -> Result<OwnedSemaphorePermit> {
        validate()?;
        let waiting = self.audio_waiters.clone().try_acquire_owned().map_err(|_| {
            crate::diagnostics::native_event("audio_queue_full", &[("limit", AUDIO_WAITING_CAPACITY as u64)]);
            AppError::new(
                ErrorCode::Provider,
                "Another recording is waiting for AI capacity. This recording was not submitted. Wait for transcription before recording again.",
            )
        })?;
        let started = Instant::now();
        let acquire = self.network.clone().acquire_owned();
        tokio::pin!(acquire);
        let permit = loop {
            tokio::select! {
                result = &mut acquire => break result.map_err(|_| AppError::new(
                    ErrorCode::Internal, "AI request admission is unavailable."
                ))?,
                _ = tokio::time::sleep(std::time::Duration::from_millis(100)) => validate()?,
            }
        };
        // Settings/source changes while queued must defeat dispatch even if the
        // permit and invalidation become ready together.
        validate()?;
        drop(waiting);
        let waited = started.elapsed();
        if waited >= Duration::from_millis(100) {
            crate::diagnostics::native_event(
                "audio_capacity_wait",
                &[
                    (
                        "wait_ms",
                        u64::try_from(waited.as_millis()).unwrap_or(u64::MAX),
                    ),
                    ("limit", NETWORK_CAPACITY as u64),
                ],
            );
        }
        Ok(permit)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::time::Duration;

    #[tokio::test]
    async fn abandoned_keychain_awaits_retain_capacity_until_blocking_reads_finish() {
        let reads = Arc::new(CredentialReads::new());
        let mut releases = Vec::new();
        for _ in 0..NETWORK_CAPACITY {
            let (started, waiting) = tokio::sync::oneshot::channel();
            let (release, blocked) = std::sync::mpsc::channel();
            releases.push(release);
            let owned = reads.clone();
            let task = tokio::spawn(async move {
                owned
                    .read(move || {
                        started.send(()).unwrap();
                        blocked.recv().unwrap();
                        Ok(())
                    })
                    .await
            });
            tokio::time::timeout(Duration::from_secs(2), waiting)
                .await
                .unwrap()
                .unwrap();
            task.abort();
            assert!(task.await.unwrap_err().is_cancelled());
        }
        let additional_calls = Arc::new(std::sync::atomic::AtomicUsize::new(0));
        for _ in 0..20 {
            let counter = additional_calls.clone();
            let result = reads
                .read(move || {
                    counter.fetch_add(1, Ordering::SeqCst);
                    Ok(())
                })
                .await;
            assert_eq!(result.unwrap_err().code, ErrorCode::AdmissionHeld);
        }
        assert_eq!(additional_calls.load(Ordering::SeqCst), 0);
        assert_eq!(reads.slots.available_permits(), 0);
        for release in releases {
            release.send(()).unwrap();
        }
        tokio::time::timeout(Duration::from_secs(2), async {
            while reads.slots.available_permits() != NETWORK_CAPACITY {
                tokio::task::yield_now().await;
            }
        })
        .await
        .unwrap();
        assert_eq!(reads.read(|| Ok(42)).await.unwrap(), 42);
        assert_eq!(reads.slots.available_permits(), NETWORK_CAPACITY);
        assert!(
            reads
                .read::<()>(|| Err(AppError::new(ErrorCode::Credential, "Synthetic failure")))
                .await
                .is_err()
        );
        assert_eq!(reads.slots.available_permits(), NETWORK_CAPACITY);
    }

    #[tokio::test]
    async fn mixed_work_shares_capacity_and_waiting_audio_is_not_starved() {
        let admission = Admission::with_capacity(2);
        let persona = admission.try_chat().unwrap();
        let coach = admission.try_chat().unwrap();
        let audio = admission.audio(|| Ok(()));
        tokio::pin!(audio);
        assert!(
            tokio::time::timeout(Duration::from_millis(10), &mut audio)
                .await
                .is_err()
        );
        assert!(admission.try_chat().is_none());
        assert!(admission.audio(|| Ok(())).await.is_err());
        drop(persona);
        // Tokio reserves released capacity for the existing waiter.
        assert!(admission.try_chat().is_none());
        let transcription = audio.await.unwrap();
        assert!(admission.try_chat().is_none());
        drop(coach);
        let another_route = admission.try_chat().unwrap();
        assert!(admission.try_chat().is_none());
        drop((transcription, another_route));
        assert!(admission.try_chat().is_some());
    }

    #[tokio::test]
    async fn invalidation_while_waiting_releases_queue_slot_without_dispatch() {
        let admission = Admission::with_capacity(2);
        let first = admission.try_chat().unwrap();
        let second = admission.try_chat().unwrap();
        let valid = AtomicBool::new(true);
        let audio = admission.audio(|| {
            if valid.load(Ordering::SeqCst) {
                Ok(())
            } else {
                Err(AppError::new(ErrorCode::Conflict, "Connection changed."))
            }
        });
        tokio::pin!(audio);
        assert!(
            tokio::time::timeout(Duration::from_millis(10), &mut audio)
                .await
                .is_err()
        );
        valid.store(false, Ordering::SeqCst);
        assert!(audio.await.is_err());
        drop((first, second));
        assert!(admission.audio(|| Ok(())).await.is_ok());
    }

    #[tokio::test]
    async fn dropping_waiter_or_active_request_releases_capacity() {
        let admission = Admission::with_capacity(2);
        let first = admission.try_chat().unwrap();
        let second = admission.try_chat().unwrap();
        // Timeout owns and drops this future, unlike the borrowed future above.
        assert!(
            tokio::time::timeout(Duration::from_millis(10), admission.audio(|| Ok(())))
                .await
                .is_err()
        );
        drop((first, second));
        let audio = admission.audio(|| Ok(())).await.unwrap();
        let chat = admission.try_chat().unwrap();
        assert!(admission.try_chat().is_none());
        drop(audio);
        assert!(admission.try_chat().is_some());
        drop(chat);
    }

    #[test]
    fn production_capacity_is_shared_and_reusable() {
        let admission = Admission::new();
        let permits: Vec<_> = (0..NETWORK_CAPACITY)
            .map(|_| admission.try_chat().unwrap())
            .collect();
        assert!(admission.try_chat().is_none());
        drop(permits);
        let permits: Vec<_> = (0..NETWORK_CAPACITY)
            .map(|_| admission.try_chat().unwrap())
            .collect();
        assert_eq!(permits.len(), NETWORK_CAPACITY);
    }
}
