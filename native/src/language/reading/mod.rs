//! Explicit reading consumers. Generated payloads are evictable local results;
//! redacted execution receipts are durable. New text help can earn exploration, never skill XP.
pub(crate) mod explanation_graph;
pub(crate) mod guide_graph;
pub(crate) mod native;
pub(crate) mod native_cache;
pub(crate) mod preloaded;
mod receipts;
pub(crate) mod saved;
mod sentence_blanks;
#[cfg(test)]
mod tests;
pub(crate) mod text;
pub(crate) mod text_sources;
use crate::{
    ai::connections::access, language::gloss, learning::coaching::conversation_support as support,
    model::*, storage::store::Store,
};
use serde::{Deserialize, Serialize};
use std::{
    collections::HashMap,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
    time::Instant,
};
use ts_rs::TS;

#[derive(Clone, Debug, Deserialize, Serialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ReadingScope {
    pub language: String,
    pub variety: Option<String>,
    pub explanation: String,
    pub explanation_variety: Option<String>,
}
/// The longest selection, in UTF-16 units, one reading request accepts.
const TEXT_LIMIT: usize = 2048;
/// Which reading aid an explicit request asks for. Each aid runs the same
/// capability contract that conversation turns use for it.
#[derive(Clone, Copy, Debug, Deserialize, Serialize, TS, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ReadingAid {
    WordGloss,
    Speech,
    Translation,
    Explanations,
    Completions,
}
impl ReadingAid {
    /// The receipt kind recorded for this aid.
    pub fn receipt_kind(self) -> &'static str {
        match self {
            ReadingAid::WordGloss => "reading_gloss",
            ReadingAid::Speech => "token_speech",
            ReadingAid::Translation => "reading_translation",
            ReadingAid::Explanations => "reading_explanations",
            ReadingAid::Completions => "reading_completions",
        }
    }
    /// The task role that selects this aid's model, shared with conversation turns.
    fn role(self) -> &'static str {
        match self {
            ReadingAid::WordGloss => gloss::ROLE,
            ReadingAid::Speech => crate::ai::connections::model_routing::SPEECH_ROLE,
            ReadingAid::Translation => crate::language::translation::ROLE,
            ReadingAid::Explanations | ReadingAid::Completions => support::ROLE,
        }
    }
    fn capability(self) -> access::Capability {
        match self {
            ReadingAid::Speech => access::Capability::Speech,
            ReadingAid::WordGloss
            | ReadingAid::Translation
            | ReadingAid::Explanations
            | ReadingAid::Completions => access::Capability::Chat,
        }
    }
}
#[derive(Clone, Debug, Deserialize, Serialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ReadingInput {
    /// Optional conversation attribution captured by the reading surface.
    #[serde(default)]
    #[ts(optional)]
    pub conversation_id: Option<String>,
    // Optional durable reference owner; ordinary reading aids remain ephemeral.
    #[serde(default)]
    #[ts(optional)]
    pub reference_item: Option<String>,
    pub text: String,
    pub language: String,
    pub variety: Option<String>,
    pub explanation: String,
    pub explanation_variety: Option<String>,
    pub aid: ReadingAid,
}
#[derive(Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct ReadingResult {
    pub gloss: Option<WordGlossView>,
    pub audio_base64: Option<String>,
    pub audio_alignment: Option<crate::speech::alignment::SpeechAlignment>,
    pub translation: Option<String>,
    pub explanations: Option<support::ReplyExplanations>,
    #[ts(type = "unknown")]
    pub receipt: serde_json::Value,
}

pub struct Request {
    pub id: String,
    pub fresh: bool,
    pub input: ReadingInput,
    pub context: crate::configuration::LanguageContext,
    pub target: access::ResolvedTarget,
    /// The model this aid runs on, selected by its task role.
    pub model: String,
    pub(crate) model_role: &'static str,
    pub install: String,
    pub attempt: String,
    pub operation: String,
    pub(crate) config_hash: String,
    created: Instant,
    claimed: AtomicBool,
    cancelled: AtomicBool,
}
impl Request {
    pub fn capture(store: &Store, input: ReadingInput) -> Result<Self> {
        crate::learning::effort::exploration::validate_conversation(
            &store.connection,
            input.conversation_id.as_deref(),
            &input.language,
        )?;
        if input.text.trim().is_empty()
            || input.text.encode_utf16().count() > TEXT_LIMIT
            || input.text.contains('\0')
        {
            return Err(AppError::new(
                ErrorCode::Validation,
                format!("Select between 1 and {TEXT_LIMIT} characters for reading help."),
            ));
        }
        if input.aid == ReadingAid::Completions && !sentence_blanks::contains(&input.text, true) {
            return Err(AppError::new(
                ErrorCode::Validation,
                "Sentence completion requires a template with an underscore slot.",
            ));
        }
        let context = store.config.resolve_pair(
            &input.language,
            input.variety.as_deref(),
            &input.explanation,
            input.explanation_variety.as_deref(),
        )?;
        Self::capture_context(store, input, context)
    }

    /// Conversation owners have already validated their message size and capture
    /// language context with the turn. They share execution without changing it.
    pub(crate) fn capture_context(
        store: &Store,
        input: ReadingInput,
        context: crate::configuration::LanguageContext,
    ) -> Result<Self> {
        let target = crate::ai::connections::speech_routing::resolve(
            &store.connection,
            input.aid.capability(),
            &context,
        )?;
        // The model a request runs on follows the same task roles as conversation
        // turns; access validation still compares against the resolved target.
        let model = crate::ai::connections::model_routing::target(
            &target,
            input.aid.role(),
            &crate::ai::connections::configuration::config(&store.connection)?.fast_model,
        )
        .model;
        let (attempt, operation) = crate::ai::identity::new_execution_ids();
        let request = Self {
            id: uuid::Uuid::new_v4().to_string(),
            fresh: false,
            model_role: input.aid.role(),
            input,
            context,
            target,
            model,
            install: store.learner_id()?,
            attempt,
            operation,
            config_hash: store.config.hash().into(),
            created: Instant::now(),
            claimed: AtomicBool::new(false),
            cancelled: AtomicBool::new(false),
        };
        request.validate_source(store)?;
        Ok(request)
    }
    pub fn validate_source(&self, store: &Store) -> Result<()> {
        crate::learning::effort::exploration::validate_conversation(
            &store.connection,
            self.input.conversation_id.as_deref(),
            &self.input.language,
        )?;
        crate::drill::reference::validate(store, self)?;
        if self.cancelled.load(Ordering::SeqCst) {
            return Err(self.stopped("cancelled"));
        }
        if self.input.aid != ReadingAid::Speech {
            self.validate_access(store)?;
        } else if self.install != store.learner_id()? {
            return Err(self.stopped("workspace_changed"));
        }
        Ok(())
    }
    pub(crate) fn validate_execution(&self, store: &Store) -> Result<()> {
        self.validate_access(store)?;
        if crate::ai::connections::configuration::config(&store.connection)?.paused {
            return Err(self.stopped("paused"));
        }
        Ok(())
    }
    pub(crate) fn validate_access(&self, store: &Store) -> Result<()> {
        if self.install != store.learner_id()? {
            return Err(self.stopped("workspace_changed"));
        }
        if self.config_hash != store.config.hash() {
            return Err(self.stopped("configuration_changed"));
        }
        let current = crate::ai::connections::speech_routing::resolve(
            &store.connection,
            self.input.aid.capability(),
            &self.context,
        )?;
        let config = crate::ai::connections::configuration::config(&store.connection)?;
        let model = crate::ai::connections::model_routing::target(
            &current,
            self.model_role,
            &config.fast_model,
        )
        .model;
        if current.revision != self.target.revision
            || current.route != self.target.route
            || current.url != self.target.url
            || current.model != self.target.model
            || current.credential != self.target.credential
            || model != self.model
        {
            return Err(self.stopped("access_changed"));
        }
        Ok(())
    }
    fn stopped(&self, reason: &str) -> AppError {
        AppError::new(ErrorCode::Conflict, "Reading request stopped or its source/access changed. See the shared execution receipt for any submitted work.")
            .with_diagnostics(serde_json::json!({"stage":"reading_authority", "reason":reason, "dispatched":null}))
    }
    fn source(&self) -> gloss::Source {
        gloss::Source {
            identity: crate::language::linguistics::SourceIdentity {
                message_id: crate::ai::results::digest(
                    &serde_json::to_vec(&serde_json::json!([self.input.text, self.context]))
                        .expect("serializable reading source"),
                ),
                target_language_id: self.input.language.clone(),
                explanation_language_id: self.input.explanation.clone(),
                analysis_version: crate::language::linguistics::ANALYSIS_VERSION.into(),
            },
            text: self.input.text.clone(),
        }
    }
    /// A speech request through the persona speech contract: the same source
    /// limits, voice, language label and route validation.
    pub(crate) fn speech_input(&self) -> Result<crate::ai::audio::SpeechInput> {
        crate::ai::audio::speech_input(
            &self.target,
            self.input.text.clone(),
            crate::configuration::SPEECH_VOICE.into(),
            &self.context,
        )
    }
}
#[derive(Default)]
pub struct Registry(Mutex<HashMap<String, Arc<Request>>>);
impl Registry {
    #[cfg(test)]
    pub fn begin(&self, store: &Store, input: ReadingInput) -> Result<String> {
        self.begin_fresh(store, input, false)
    }
    pub fn begin_fresh(&self, store: &Store, input: ReadingInput, fresh: bool) -> Result<String> {
        let mut entries = self
            .0
            .lock()
            .map_err(|_| crate::diagnostics::failures::poisoned(crate::application::internal()))?;
        let expired: Vec<_> = entries
            .values()
            .filter(|r| !r.claimed.load(Ordering::SeqCst) && r.created.elapsed().as_secs() >= 30)
            .map(|r| r.id.clone())
            .collect();
        for id in expired {
            receipts::cancel(store, &id)?;
            entries.remove(&id);
        }
        // This registry owns local consumers, not provider admission. Independent
        // open helpers must not prevent another local request from being created.
        let mut request = Request::capture(store, input)?;
        request.fresh = fresh;
        let request = Arc::new(request);
        receipts::begin(store, &request)?;
        let id = request.id.clone();
        entries.insert(id.clone(), request);
        Ok(id)
    }
    pub fn claim(&self, id: &str) -> Result<Arc<Request>> {
        let entries = self
            .0
            .lock()
            .map_err(|_| crate::diagnostics::failures::poisoned(crate::application::internal()))?;
        let request = entries.get(id).ok_or_else(|| {
            AppError::new(
                ErrorCode::Conflict,
                "Reading request expired or was closed.",
            )
        })?;
        if request.created.elapsed().as_secs() >= 30 {
            return Err(request.stopped("expired"));
        }
        if request.cancelled.load(Ordering::SeqCst) {
            return Err(request.stopped("cancelled"));
        }
        if request.claimed.swap(true, Ordering::SeqCst) {
            return Err(request.stopped("already_claimed"));
        }
        Ok(request.clone())
    }
    /// Validate a live consumer without claiming it a second time.
    pub(crate) fn validate_native(&self, store: &Store, id: &str) -> Result<()> {
        let entries = self
            .0
            .lock()
            .map_err(|_| crate::diagnostics::failures::poisoned(crate::application::internal()))?;
        let request = entries.get(id).ok_or_else(|| {
            AppError::new(ErrorCode::Conflict, "Reading consumer is no longer active.")
        })?;
        request.validate_source(store)?;
        request.validate_execution(store)
    }
    pub fn cancel(&self, store: &Store, id: &str) -> Result<()> {
        let mut entries = self
            .0
            .lock()
            .map_err(|_| crate::diagnostics::failures::poisoned(crate::application::internal()))?;
        if let Some(request) = entries.get(id) {
            request.cancelled.store(true, Ordering::SeqCst);
            receipts::cancel(store, id)?;
            if !request.claimed.load(Ordering::SeqCst) {
                entries.remove(id);
            }
        }
        Ok(())
    }
    pub fn remove(&self, id: &str) -> Result<()> {
        self.0
            .lock()
            .map_err(|_| crate::diagnostics::failures::poisoned(crate::application::internal()))?
            .remove(id);
        Ok(())
    }
}
pub use receipts::{activity, finish, recover};
