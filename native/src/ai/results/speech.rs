//! Local speech identity uses exact wire inputs and access scope, never presentation owners.
use super::*;
use crate::ai::{audio::SpeechInput, connections::access::ResolvedTarget};
use serde_json::json;

pub fn scope(target: &ResolvedTarget, install: &str) -> Result<String> {
    Ok(digest(&serde_json::to_vec(&json!([
        install,
        target.route,
        target.url,
        target.model,
        target.credential
    ]))?))
}
pub fn request_key(scope: &str, input: &SpeechInput) -> Result<String> {
    Ok(digest(&serde_json::to_vec(&json!([
        "speech-local-v3",
        scope,
        input.text,
        input.language,
        input.language_tag
    ]))?))
}
