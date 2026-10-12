use super::*;

pub(crate) struct Application {
    pub(super) scheduler_idle: Mutex<super::scheduler::Idle>,
    pub(super) conversation_reads: Mutex<super::conversation_reads::Cache>,
    inspection_cache: Mutex<super::response_cache::Cache>,
    pub(super) learning_reads: Mutex<super::learning_reads::Cache>,
    #[cfg(feature = "workflow-capture")]
    pub(super) local_timings: super::local_timing::Measurements,
    pub(super) scheduler_wake: tokio::sync::Notify,
    pub(super) conversation_changed: tokio::sync::Notify,
    /// Protected reports whose storage acknowledgement failed. Never diagnostics
    /// and never automatically resubmitted; reset explicitly clears this buffer.
    pub(super) graph_reports: Mutex<Vec<crate::ai::graph::InvocationReport>>,
    pub(super) reading_pending: crate::ai::results::pending::Registry<crate::ai::results::Retained>,
    pub(super) transcription_pending:
        crate::ai::results::pending::Registry<crate::ai::results::Retained>,
    pub(super) speech_streams: Mutex<crate::speech::stream_delivery::Registry>,
    pub(super) native_transcription_hosts: Mutex<super::native_transcription::Hosts>,
    pub(super) native_speech_hosts: Mutex<super::native_speech::Hosts>,
    pub(super) native_audio_slots: tokio::sync::Semaphore,
    pub(super) speech_pending: crate::ai::results::pending::Registry<crate::ai::results::Retained>,
    pub(super) reading: crate::language::reading::Registry,
    pub(crate) admission: admission::Admission,
    pub(super) generations: generation::Registry,
    pub(crate) listening: Mutex<Option<Arc<crate::speech::recording::continuous::Session>>>,
    pub(crate) microphone_test: Mutex<Option<crate::speech::recording::microphone_test::Session>>,
    pub(crate) capture: Mutex<Option<voice::Recording>>,
    /// Current manual take identity and its failed audio, if available for Retry.
    pub(crate) failed_take: Mutex<voice::RetryTake>,
    pub(crate) store: Mutex<Option<Store>>,
    /// Why the workspace could not be opened at startup. Commands report it
    /// rather than a generic failure, and the window stays open so the reason
    /// reaches the screen and the reset stays reachable.
    pub(super) refusal: Mutex<Option<AppError>>,
    /// A cleanup a previous reset recorded that this launch could not finish.
    pub(super) cleanup: Mutex<Option<AppError>>,
    pub(super) credential_cleanup: Mutex<Option<AppError>>,
    pub(super) credential_operations: Mutex<()>,
    pub(super) fatal: Mutex<Option<AppError>>,
    pub(super) signing_in: tokio::sync::Mutex<()>,
    pub(super) auth_epoch: std::sync::atomic::AtomicU64,
    /// The AI View's place, kept while it moves between the panel and its window.
    pub(super) ai_view_selection: Mutex<Option<crate::model::AiViewSelection>>,
}
pub(crate) struct StoreGuard<'a> {
    store: Option<MutexGuard<'a, Option<Store>>>,
    #[cfg(feature = "workflow-capture")]
    timing: Option<super::local_timing::Pending<'a>>,
}
impl Deref for StoreGuard<'_> {
    type Target = Store;
    fn deref(&self) -> &Self::Target {
        self.store
            .as_ref()
            .expect("store guard released")
            .as_ref()
            .expect("application store is unavailable")
    }
}
impl DerefMut for StoreGuard<'_> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.store
            .as_mut()
            .expect("store guard released")
            .as_mut()
            .expect("application store is unavailable")
    }
}
#[cfg(feature = "workflow-capture")]
impl Drop for StoreGuard<'_> {
    fn drop(&mut self) {
        let released = std::time::Instant::now();
        drop(self.store.take());
        if let Some(timing) = self.timing.take() {
            timing.released(released);
        }
    }
}
impl Application {
    pub(crate) fn inspect_cached<T: serde::Serialize + serde::de::DeserializeOwned>(
        &self,
        key: String,
        read: impl FnOnce() -> Result<T>,
    ) -> Result<T> {
        let cached = self
            .inspection_cache
            .lock()
            .map_err(|_| internal())?
            .get(&key);
        if let Some(bytes) = cached {
            return Ok(serde_json::from_slice(&bytes)?);
        }
        let value = read()?;
        let bytes = serde_json::to_vec(&value)?;
        self.inspection_cache
            .lock()
            .map_err(|_| internal())?
            .insert(key, bytes);
        Ok(value)
    }
    pub(crate) fn clear_graph_reports(&self) {
        *self
            .learning_reads
            .lock()
            .unwrap_or_else(|p| p.into_inner()) = Default::default();
        *self
            .scheduler_idle
            .lock()
            .unwrap_or_else(|p| p.into_inner()) = Default::default();
        *self
            .conversation_reads
            .lock()
            .unwrap_or_else(|p| p.into_inner()) = Default::default();
        *self
            .inspection_cache
            .lock()
            .unwrap_or_else(|p| p.into_inner()) = Default::default();
        self.graph_reports
            .lock()
            .unwrap_or_else(|poison| poison.into_inner())
            .clear();
    }

    /// Open the workspace without ever aborting the launch. A refused database is
    /// an ordinary outcome, and it has to reach a screen that can reset it.
    /// `cleanup` is the failure, if any, of finishing a previous reset; the caller
    /// runs that before the log sink opens, because the log directory is part of it.
    #[cfg(test)]
    pub(crate) fn recording_fixture(workspace: &std::path::Path) -> Arc<Self> {
        Self::start(workspace, None)
    }
    pub(super) fn start(workspace: &std::path::Path, cleanup: Option<AppError>) -> Arc<Self> {
        let (store, refusal) = match Store::open(workspace) {
            Ok(store) => (Some(store), None),
            Err(error) => (None, Some(error)),
        };
        Arc::new(Self {
            scheduler_idle: Default::default(),
            conversation_reads: Default::default(),
            inspection_cache: Default::default(),
            learning_reads: Default::default(),
            #[cfg(feature = "workflow-capture")]
            local_timings: Default::default(),
            scheduler_wake: Default::default(),
            conversation_changed: Default::default(),
            graph_reports: Mutex::new(Vec::new()),
            speech_streams: Default::default(),
            speech_pending: Default::default(),
            native_transcription_hosts: Default::default(),
            native_speech_hosts: Default::default(),
            native_audio_slots: tokio::sync::Semaphore::new(
                crate::speech::delivery::DELIVERY_ENTRIES,
            ),
            transcription_pending: Default::default(),
            reading_pending: Default::default(),
            admission: admission::Admission::new(),
            reading: Default::default(),
            generations: generation::Registry::default(),
            microphone_test: Mutex::new(None),
            capture: Mutex::new(None),
            failed_take: Default::default(),
            listening: Mutex::new(None),
            store: Mutex::new(store),
            refusal: Mutex::new(refusal),
            cleanup: Mutex::new(cleanup),
            credential_cleanup: Mutex::new(None),
            credential_operations: Mutex::new(()),
            fatal: Mutex::new(None),
            signing_in: tokio::sync::Mutex::new(()),
            auth_epoch: std::sync::atomic::AtomicU64::new(0),
            ai_view_selection: Mutex::new(None),
        })
    }
    fn snapshot_error(slot: &Mutex<Option<AppError>>) -> Option<AppError> {
        match slot.lock() {
            Ok(value) => value.clone(),
            Err(_) => {
                let error = crate::diagnostics::failures::poisoned(internal());
                crate::diagnostics::failures::report("startup_state", &error);
                Some(error)
            }
        }
    }
    pub(super) fn refusal(&self) -> Option<AppError> {
        Self::snapshot_error(&self.refusal)
    }
    pub(super) fn startup_state(&self) -> StartupState {
        StartupState {
            refusal: self.refusal(),
            cleanup: Self::snapshot_error(&self.cleanup),
            credential_cleanup: Self::snapshot_error(&self.credential_cleanup),
        }
    }
    #[track_caller]
    pub(crate) fn lock(&self) -> Result<StoreGuard<'_>> {
        #[cfg(feature = "workflow-capture")]
        let started = std::time::Instant::now();
        if let Some(error) = self
            .fatal
            .lock()
            .map_err(|_| crate::diagnostics::failures::poisoned(internal()))?
            .as_ref()
        {
            return Err(error.clone());
        }
        let store = self
            .store
            .lock()
            .map_err(|_| crate::diagnostics::failures::poisoned(internal()))?;
        #[cfg(feature = "workflow-capture")]
        let timing = self
            .local_timings
            .acquired(std::panic::Location::caller(), started);
        if store.is_none() {
            if let Some(error) = self
                .refusal
                .lock()
                .map_err(|_| crate::diagnostics::failures::poisoned(internal()))?
                .as_ref()
            {
                return Err(error.clone());
            }
            return Err(internal());
        }
        Ok(StoreGuard {
            store: Some(store),
            #[cfg(feature = "workflow-capture")]
            timing,
        })
    }
    pub(crate) fn credential_operation(&self) -> Result<MutexGuard<'_, ()>> {
        self.credential_operations
            .lock()
            .map_err(|_| crate::diagnostics::failures::poisoned(internal()))
    }
    fn clean_credentials_with(&self, remove: impl Fn(&str) -> Result<()>) -> Result<()> {
        loop {
            let id = self.lock()?.claim_credential_cleanup()?;
            let Some(id) = id else {
                return Ok(());
            };
            // A claimed unique ID cannot be reused while the external call blocks.
            let result = remove(&id);
            self.lock()?
                .finish_credential_cleanup(&id, result.is_ok())?;
            result?;
        }
    }
    pub(crate) fn clean_credentials(&self) -> Result<()> {
        self.clean_credentials_with(credentials::remove)
    }
    pub(super) fn recover_credential_cleanup_with(
        &self,
        remove: impl Fn(&str) -> Result<()>,
    ) -> Result<StartupState> {
        let _operation = self.credential_operation()?;
        let error = match self.clean_credentials_with(remove) {
            Ok(()) => None,
            Err(error) if error.code == ErrorCode::Credential => Some(error),
            Err(error) => return Err(error),
        };
        *self
            .credential_cleanup
            .lock()
            .map_err(|_| crate::diagnostics::failures::poisoned(internal()))? = error;
        Ok(self.startup_state())
    }
    pub(crate) fn write_credential_with<T>(
        &self,
        prepare: impl FnOnce(&mut Store) -> Result<()>,
        write: impl FnOnce(&str) -> Result<()>,
        commit: impl FnOnce(&mut Store, &str) -> Result<T>,
        remove: impl Fn(&str) -> Result<()>,
    ) -> Result<T> {
        let _credential_operation = self.credential_operation()?;
        let id = uuid::Uuid::new_v4().to_string();
        {
            let mut store = self.lock()?;
            prepare(&mut store)?;
            // Durable before the secret exists: a reset that cannot read the
            // database still has to be able to find every keychain entry.
            credentials::remember(&store.credential_index, &id)?;
            store.reserve_credential(&id)?;
        }
        let written = write(&id);
        let result = {
            let mut store = self.lock()?;
            store.credential_writes.remove(&id);
            written.and_then(|_| commit(&mut store, &id))
        };
        self.clean_credentials_with(remove)?;
        result
    }
    pub(crate) fn stop(&self, error: AppError) {
        crate::diagnostics::failures::report("scheduler_stopped", &error);
        *self.fatal.lock().expect("execution fault mutex") = Some(error);
    }
}

#[cfg(test)]
#[path = "tests/capture.rs"]
mod capture_tests;

#[cfg(test)]
#[path = "tests/startup_format.rs"]
mod startup_format_tests;

#[cfg(test)]
#[path = "tests/credential_io.rs"]
mod tests;
