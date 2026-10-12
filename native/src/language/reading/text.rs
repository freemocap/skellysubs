//! Typed reading contracts and validated, evictable results. No view ownership.
use super::*;
use crate::ai::{results, transport::provider};
use serde_json::json;

#[derive(Clone, Serialize, Deserialize)]
pub struct Stored {
    pub scope: ReadingScope,
    pub text: String,
    pub access_scope: String,
    pub gloss: Option<WordGlossView>,
    pub translation: Option<String>,
    pub explanations: Option<support::ReplyExplanations>,
    /// Original validated completion for independent conversation publication.
    /// Older evictable entries remain readable by saved-source lookup.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub completion: Option<provider::Completion>,
}
impl Stored {
    pub fn decode(payload: &[u8]) -> Result<Self> {
        serde_json::from_slice(payload).map_err(|_| {
            AppError::new(
                ErrorCode::Storage,
                "Saved reading result has an invalid shape.",
            )
        })
    }
}
impl Request {
    pub fn text_scope(&self) -> Result<String> {
        access_scope(&self.install, &self.target, &self.model, &self.config_hash)
    }
}

fn access_scope(
    install: &str,
    target: &access::ResolvedTarget,
    model: &str,
    config_hash: &str,
) -> Result<String> {
    Ok(results::digest(&serde_json::to_vec(&json!([
        install,
        target.route,
        target.url,
        target.credential,
        model,
        config_hash
    ]))?))
}
pub(super) fn current_scope(store: &Store) -> Result<String> {
    let target = access::resolve(&store.connection, access::Capability::Chat)?;
    let model = crate::ai::connections::model_routing::target(
        &target,
        ReadingAid::WordGloss.role(),
        &crate::ai::connections::configuration::config(&store.connection)?.fast_model,
    )
    .model;
    access_scope(&store.learner_id()?, &target, &model, store.config.hash())
}
