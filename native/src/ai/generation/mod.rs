//! Reviewable proposal ownership, separate from shared provider execution.
//! Persona proposals stay volatile; durable Drill candidates belong to previews.
pub(crate) mod generation_receipts;
pub(crate) mod graph;
use crate::ai::connections::access;
use crate::model::*;
use crate::storage::store::Store;
use std::{
    collections::HashMap,
    future::Future,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
    time::{Duration, Instant},
};
use tokio::sync::Notify;

const CAPACITY: usize = 4;
const PENDING_TTL: Duration = Duration::from_secs(30);

pub struct Request {
    pub id: String,
    pub kind: &'static str,
    pub language_id: String,
    pub language: Language,
    pub config_hash: String,
    pub language_context: crate::configuration::LanguageContext,
    pub brief: Option<String>,
    pub target: access::ResolvedTarget,
    pub credential: String,
    pub install_id: String,
    pub attempt: String,
    pub operation: String,
    created: Instant,
    claimed: AtomicBool,
    submitted: AtomicBool,
    cancelled: AtomicBool,
    changed: Notify,
}
impl Request {
    pub(crate) fn capture_context(
        store: &Store,
        kind: &'static str,
        context: crate::configuration::LanguageContext,
        brief: Option<String>,
    ) -> Result<Self> {
        let language_id = context.language_id.clone();
        let language = store.config.language(&language_id)?;
        let target = access::resolve(&store.connection, access::Capability::Chat)?;
        let credential =
            crate::ai::connections::configuration::active_credential(&store.connection)?
                .ok_or_else(|| {
                    AppError::new(
                        ErrorCode::Credential,
                        "Configure the selected AI connection before generating.",
                    )
                })?;
        let (attempt, operation) = crate::ai::identity::new_execution_ids();
        let request = Self {
            id: uuid::Uuid::new_v4().to_string(),
            language_context: context,
            kind,
            language_id,
            language,
            config_hash: store.config.hash().into(),
            brief,
            target,
            credential,
            install_id: store.learner_id()?,
            attempt,
            operation,
            created: Instant::now(),
            claimed: AtomicBool::new(false),
            submitted: AtomicBool::new(false),
            cancelled: AtomicBool::new(false),
            changed: Notify::new(),
        };
        request.validate(store)?;
        Ok(request)
    }
    pub fn was_submitted(&self) -> bool {
        self.submitted.load(Ordering::SeqCst)
    }
    pub fn mark_submitted(&self) {
        self.submitted.store(true, Ordering::SeqCst);
    }
    pub(crate) fn cancel(&self) {
        self.cancelled.store(true, Ordering::SeqCst);
        self.changed.notify_one();
    }
    fn stopped(&self, reason: &str) -> AppError {
        if self.was_submitted() {
            AppError::new(
                ErrorCode::UnknownOutcome,
                format!(
                    "Generation stopped locally: {reason} Provider outcome and billing may be unknown. No automatic retry was made."
                ),
            )
        } else {
            AppError::new(
                ErrorCode::Conflict,
                format!("Generation was not submitted: {reason}"),
            )
        }
    }
    fn check_cancelled(&self) -> Result<()> {
        if self.cancelled.load(Ordering::SeqCst) {
            Err(self.stopped("cancelled"))
        } else {
            Ok(())
        }
    }
    pub fn validate(&self, store: &Store) -> Result<()> {
        self.check_cancelled()?;
        self.validate_execution(store)?;
        self.check_cancelled()
    }
    /// Submission authority is independent of a closed proposal after dispatch.
    pub(crate) fn validate_execution(&self, store: &Store) -> Result<()> {
        if self.install_id != store.learner_id()? || self.config_hash != store.config.hash() {
            return Err(self.stopped("language configuration changed"));
        }
        let config = crate::ai::connections::configuration::config(&store.connection)?;
        if config.paused {
            return Err(if self.was_submitted() {
                self.stopped("AI execution is paused")
            } else {
                AppError::new(
                    ErrorCode::AdmissionHeld,
                    "AI execution is paused. Resume AI execution before generating.",
                )
            });
        }
        if config.revision != self.target.revision
            || config.route != self.target.route
            || crate::ai::connections::configuration::active_credential(&store.connection)?
                .as_deref()
                != Some(&self.credential)
        {
            return Err(self.stopped("connection authority changed"));
        }
        let current = access::resolve(&store.connection, access::Capability::Chat)?;
        if current.url != self.target.url
            || current.model != self.target.model
            || current.credential != self.target.credential
        {
            return Err(self.stopped("connection authority changed"));
        }
        Ok(())
    }
}

/// Preserve the refusal and pause related queued work without blocking new requests.
pub fn accept_completion(
    store: &mut Store,
    request: &Request,
    outcome: &Result<crate::ai::transport::provider::Completion>,
) -> Result<()> {
    match outcome {
        Err(error) => {
            store.note_refusal(&request.target, error)?;
            Err(error.clone())
        }
        Ok(completion) => {
            request.validate_execution(store)?;
            if completion.finish_reason == "error" {
                return Err(AppError::new(
                    ErrorCode::Provider,
                    "The AI service did not finish the generation. Usage may have been incurred; no automatic retry was made.",
                ).with_diagnostics(serde_json::json!({"stage":"provider_completion","chars":completion.text.chars().count(),"response":completion.diagnostics})));
            }
            Ok(())
        }
    }
}

#[derive(Default)]
pub struct Registry {
    requests: Mutex<HashMap<String, Arc<Request>>>,
}
impl Registry {
    pub(crate) fn native_request(&self, id: &str) -> Result<Arc<Request>> {
        self.requests
            .lock()
            .map_err(|_| crate::diagnostics::failures::poisoned(crate::application::internal()))?
            .get(id)
            .cloned()
            .ok_or_else(|| {
                AppError::new(
                    ErrorCode::Conflict,
                    "Proposal ownership is no longer active.",
                )
            })
    }

    /// Return expired ownership for the caller's durable terminal receipts.
    pub fn expire(&self) -> Result<Vec<Arc<Request>>> {
        let mut requests = self
            .requests
            .lock()
            .map_err(|_| crate::diagnostics::failures::poisoned(crate::application::internal()))?;
        let expired: Vec<_> = requests
            .values()
            .filter(|r| !r.claimed.load(Ordering::SeqCst) && r.created.elapsed() >= PENDING_TTL)
            .cloned()
            .collect();
        for request in &expired {
            requests.remove(&request.id);
        }
        Ok(expired)
    }
    pub fn insert(&self, request: Request) -> Result<Arc<Request>> {
        let mut requests = self
            .requests
            .lock()
            .map_err(|_| crate::diagnostics::failures::poisoned(crate::application::internal()))?;
        if requests.len() >= CAPACITY {
            return Err(AppError::new(
                ErrorCode::AdmissionHeld,
                "Too many generations are pending. Close a pending generation or let it finish.",
            ));
        }
        let request = Arc::new(request);
        requests.insert(request.id.clone(), request.clone());
        Ok(request)
    }
    pub fn claim(&self, id: &str) -> Result<Run<'_>> {
        self.claim_for(id, "persona")
    }
    pub(crate) fn claim_for(&self, id: &str, kind: &str) -> Result<Run<'_>> {
        let requests = self
            .requests
            .lock()
            .map_err(|_| crate::diagnostics::failures::poisoned(crate::application::internal()))?;
        let request = requests.get(id).ok_or_else(|| {
            AppError::new(
                ErrorCode::Conflict,
                "This generation is unavailable or expired. Start a new generation.",
            )
        })?;
        if request.kind != kind || request.created.elapsed() >= PENDING_TTL {
            return Err(AppError::new(
                ErrorCode::Conflict,
                "This generation request is unavailable for this task.",
            ));
        }
        if request.claimed.swap(true, Ordering::SeqCst) {
            return Err(AppError::new(
                ErrorCode::Conflict,
                "This generation has already been run.",
            ));
        }
        request.check_cancelled()?;
        Ok(Run {
            registry: self,
            request: request.clone(),
        })
    }
    pub(crate) fn cancel_for(&self, id: &str, kind: &str) -> Result<Option<Arc<Request>>> {
        {
            let requests = self.requests.lock().map_err(|_| {
                crate::diagnostics::failures::poisoned(crate::application::internal())
            })?;
            if requests.get(id).is_some_and(|request| request.kind != kind) {
                return Err(AppError::new(
                    ErrorCode::Conflict,
                    "This generation request belongs to another task.",
                ));
            }
        }
        self.cancel(id)
    }
    pub fn cancel(&self, id: &str) -> Result<Option<Arc<Request>>> {
        let mut requests = self
            .requests
            .lock()
            .map_err(|_| crate::diagnostics::failures::poisoned(crate::application::internal()))?;
        let Some(request) = requests.get(id).cloned() else {
            return Ok(None);
        };
        request.cancel();
        if !request.claimed.load(Ordering::SeqCst) {
            requests.remove(id);
        }
        Ok(Some(request))
    }
}

pub struct Run<'a> {
    registry: &'a Registry,
    pub request: Arc<Request>,
}
impl Drop for Run<'_> {
    fn drop(&mut self) {
        if let Ok(mut requests) = self.registry.requests.lock() {
            requests.remove(&self.request.id);
        }
    }
}

/// Cancels local credential/provider awaits promptly and checks mutable authority
/// even while a server is silent. The caller revalidates once more before adoption.
pub async fn await_checked<T>(
    request: &Request,
    future: impl Future<Output = T>,
    mut validate: impl FnMut() -> Result<()>,
) -> Result<T> {
    request.check_cancelled()?;
    validate()?;
    tokio::pin!(future);
    loop {
        tokio::select! {
            biased;
            _ = request.changed.notified() => { request.check_cancelled()?; },
            _ = tokio::time::sleep(Duration::from_millis(100)) => { request.check_cancelled()?; validate()?; },
            result = &mut future => return Ok(result),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::AtomicUsize;

    fn fixture() -> (tempfile::TempDir, Store) {
        let directory = tempfile::tempdir().unwrap();
        let store = Store::open(&directory.path().join("generation.sqlite3")).unwrap();
        store.connection.execute("UPDATE ai_config SET route='custom',custom_config=json_set(custom_config,'$.baseUrl','http://127.0.0.1:8765/v1','$.bearerAuth',json('false'))", []).unwrap();
        (directory, store)
    }
    fn capture(store: &Store) -> Request {
        Request::capture(store, "spanish".into(), Some("A quiet librarian".into())).unwrap()
    }

    #[test]
    fn failed_completion_preserves_rate_limit_details() {
        let (_directory, mut store) = fixture();
        let request = capture(&store);
        let completion = crate::ai::transport::provider::Completion {
            diagnostics: Some(serde_json::json!({
                "id": "request-test",
                "choices": [{"error": {"code": 429}}],
                "usage": {"total_tokens": 0}
            })),
            text: String::new(),
            finish_reason: "error".into(),
            actual_model: "test".into(),
            provider_id: "test".into(),
            input_tokens: Some(0),
            output_tokens: Some(0),
        };
        let error = accept_completion(&mut store, &request, &Ok(completion)).unwrap_err();
        assert!(error.message.starts_with("Sorry, rate limited."));
        let details = error.diagnostics.unwrap();
        assert_eq!(details["response"]["id"], "request-test");
        assert_eq!(details["response"]["choices"][0]["error"]["code"], 429);
        assert_eq!(details["response"]["usage"]["total_tokens"], 0);
    }

    #[test]
    fn wrong_task_kind_cannot_claim_or_cancel_another_request() {
        let (_directory, store) = fixture();
        let registry = Registry::default();
        let persona = registry.insert(capture(&store)).unwrap();
        assert!(registry.claim_for(&persona.id, "drill").is_err());
        assert!(registry.cancel_for(&persona.id, "drill").is_err());
        let run = registry.claim(&persona.id).unwrap();
        assert!(run.request.validate(&store).is_ok());
        let mut request = capture(&store);
        request.kind = "drill";
        let drill = registry.insert(request).unwrap();
        assert!(registry.claim(&drill.id).is_err());
        assert!(registry.cancel_for(&drill.id, "persona").is_err());
        assert!(registry.claim_for(&drill.id, "drill").is_ok());
    }

    #[test]
    fn pending_capacity_expiry_single_use_and_cancel_are_bounded() {
        let (_directory, store) = fixture();
        let registry = Registry::default();
        let mut ids = Vec::new();
        for _ in 0..CAPACITY {
            ids.push(registry.insert(capture(&store)).unwrap().id.clone());
        }
        assert!(registry.insert(capture(&store)).is_err());
        let run = registry.claim(&ids[0]).unwrap();
        assert!(registry.claim(&ids[0]).is_err());
        registry.cancel(&ids[0]).unwrap();
        assert_eq!(
            run.request.validate(&store).unwrap_err().code,
            ErrorCode::Conflict
        );
        drop(run);
        assert!(registry.claim(&ids[0]).is_err());
        registry.cancel(&ids[1]).unwrap();
        assert!(registry.claim(&ids[1]).is_err());
        let mut expired = capture(&store);
        expired.created = Instant::now() - PENDING_TTL;
        let expired = registry.insert(expired).unwrap();
        assert_eq!(registry.expire().unwrap()[0].id, expired.id);
        assert!(registry.claim(&expired.id).is_err());
        assert_eq!(registry.requests.lock().unwrap().len(), 2);
    }

    #[tokio::test]
    async fn pause_authority_and_credential_changes_during_key_read_prevent_dispatch() {
        for change in [
            "UPDATE ai_config SET paused=1",
            "UPDATE ai_config SET revision=revision+1",
            "UPDATE ai_config SET custom_credential_id='replacement',custom_config=json_set(custom_config,'$.bearerAuth',json('true'))",
        ] {
            let (_directory, store) = fixture();
            let request = capture(&store);
            let provider_calls = AtomicUsize::new(0);
            let key = await_checked(
                &request,
                async {
                    store.connection.execute(change, []).unwrap();
                    "key"
                },
                || request.validate(&store),
            )
            .await
            .unwrap();
            assert_eq!(key, "key");
            let result = async {
                request.validate(&store)?;
                request.mark_submitted();
                provider_calls.fetch_add(1, Ordering::SeqCst);
                Ok::<_, AppError>(())
            }
            .await;
            assert!(result.is_err());
            assert!(!request.was_submitted());
            assert_eq!(provider_calls.load(Ordering::SeqCst), 0);
        }
    }

    #[tokio::test]
    async fn cancellation_during_key_read_prevents_provider_dispatch() {
        let (_directory, store) = fixture();
        let registry = Registry::default();
        let request = registry.insert(capture(&store)).unwrap();
        let run = registry.claim(&request.id).unwrap();
        let waiting = await_checked(&run.request, std::future::pending::<()>(), || {
            request.validate(&store)
        });
        tokio::pin!(waiting);
        assert!(
            tokio::time::timeout(Duration::from_millis(1), &mut waiting)
                .await
                .is_err()
        );
        registry.cancel(&request.id).unwrap();
        assert_eq!(waiting.await.unwrap_err().code, ErrorCode::Conflict);
        assert!(!request.was_submitted());
    }

    #[tokio::test]
    async fn cancellation_defeats_a_simultaneously_ready_late_provider_result() {
        let (_directory, store) = fixture();
        let registry = Registry::default();
        let request = registry.insert(capture(&store)).unwrap();
        let run = registry.claim(&request.id).unwrap();
        request.mark_submitted();
        let (send, receive) = tokio::sync::oneshot::channel();
        let waiting = await_checked(&run.request, receive, || request.validate(&store));
        tokio::pin!(waiting);
        assert!(
            tokio::time::timeout(Duration::from_millis(1), &mut waiting)
                .await
                .is_err()
        );
        send.send("late proposal").unwrap();
        registry.cancel(&request.id).unwrap();
        assert_eq!(waiting.await.unwrap_err().code, ErrorCode::UnknownOutcome);
    }

    #[tokio::test]
    async fn silent_provider_is_revoked_on_pause_and_result_adoption_rechecks_authority() {
        let (_directory, store) = fixture();
        let request = capture(&store);
        request.mark_submitted();
        let waiting = await_checked(&request, std::future::pending::<()>(), || {
            request.validate(&store)
        });
        tokio::pin!(waiting);
        assert!(
            tokio::time::timeout(Duration::from_millis(1), &mut waiting)
                .await
                .is_err()
        );
        store
            .connection
            .execute("UPDATE ai_config SET paused=1", [])
            .unwrap();
        assert_eq!(
            tokio::time::timeout(Duration::from_millis(250), waiting)
                .await
                .unwrap()
                .unwrap_err()
                .code,
            ErrorCode::UnknownOutcome
        );
        store
            .connection
            .execute("UPDATE ai_config SET paused=0", [])
            .unwrap();
        let proposal = await_checked(
            &request,
            async {
                store
                    .connection
                    .execute("UPDATE ai_config SET revision=revision+1", [])
                    .unwrap();
                "proposal"
            },
            || request.validate(&store),
        )
        .await
        .unwrap();
        assert_eq!(proposal, "proposal");
        assert_eq!(
            request.validate(&store).unwrap_err().code,
            ErrorCode::UnknownOutcome
        );
    }

    #[test]
    fn own_refusal_is_preserved_without_blocking_new_generation() {
        let (_directory, mut store) = fixture();
        let request = capture(&store);
        request.mark_submitted();
        let error = AppError::new(ErrorCode::Provider, "Original provider refusal")
            .with_refusal(crate::ai::policy::refusal::classify(None, Some(60), None));
        let result = accept_completion(&mut store, &request, &Err(error.clone())).unwrap_err();
        assert_eq!(result.code, ErrorCode::Provider);
        assert_eq!(result.message, error.message);
        assert!(result.refusal.is_some());
        request.validate(&store).unwrap();
        Request::capture(&store, "spanish".into(), None).unwrap();
    }

    #[test]
    fn explicit_pause_prevents_generation_but_refusals_allow_manual_followups() {
        let (_directory, store) = fixture();
        store
            .connection
            .execute("UPDATE ai_config SET paused=1", [])
            .unwrap();
        assert!(matches!(
            Request::capture(&store, "spanish".into(), None),
            Err(AppError {
                code: ErrorCode::AdmissionHeld,
                ..
            })
        ));
        store
            .connection
            .execute("UPDATE ai_config SET paused=0", [])
            .unwrap();
        let request = capture(&store);
        let error = AppError::new(ErrorCode::Provider, "Request rate limited")
            .with_refusal(crate::ai::policy::refusal::classify(None, Some(60), None));
        let mut store = store;
        store.note_refusal(&request.target, &error).unwrap();
        Request::capture(&store, "spanish".into(), None).unwrap();
    }
}
