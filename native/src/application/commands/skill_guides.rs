//! On-open guide translation. Authored editions win; inference owns no learner credit.
use super::*;
use crate::ai::results::{self, Retained};
use crate::configuration::guide_translation::Edition;
use crate::configuration::guide_translation::SkillGuideResult;
mod cache;
mod native;
use native::produce;
use serde_json::json;

#[tauri::command]
pub(in crate::application) async fn get_skill_guide(
    state: tauri::State<'_, Arc<Application>>,
    language: String,
    variety: String,
    skill: String,
    explanation: String,
    retry: bool,
) -> Result<SkillGuideResult> {
    load(&state, &language, &variety, &skill, &explanation, retry).await
}

pub(super) async fn load(
    state: &Arc<Application>,
    language: &str,
    variety: &str,
    skill: &str,
    explanation: &str,
    retry: bool,
) -> Result<SkillGuideResult> {
    let (edition, key, request, prompt) = {
        let store = state.lock()?;
        let context = store.config.resolve(language, Some(variety), explanation)?;
        let source = store.config.guide_source(language, skill)?;
        if let Some(edition) = store.config.guide_edition(language, explanation, skill)? {
            return Ok(SkillGuideResult {
                markdown: edition.markdown(variety),
                explanation_language: explanation.into(),
                generated: false,
                context: Some(edition.context(variety)),
                provenance: json!({"sourcePaths":edition.paths,"sourceFingerprint":edition.fingerprint(),"shared":edition.shared.provenance,"guide":edition.guide.provenance}),
            });
        }
        let prompt = store.config.guide_translation_prompt().to_string();
        let key = results::digest(&serde_json::to_vec(&json!([
            "skill-guide-translation-1",
            store.learner_id()?,
            source,
            explanation,
            variety,
            prompt
        ]))?);
        if let Some(saved) = cache::lookup(&store.connection, &key)? {
            let mut result = decode(&saved)?;
            result.context = Some(source.context(variety));
            return Ok(result);
        }
        if !retry {
            cache::check_retry(&store.connection, &key, |run| {
                state.generations.native_request(run).is_ok()
            })?;
        }
        let request =
            generation::Request::capture_context(&store, "guide_translation", context, None)?;
        (source, key, request, prompt)
    };
    let context = edition.context(variety);
    let (subscription, producer) = state.reading_pending.subscribe(key.clone())?;
    if let Some(producer) = producer {
        let request = state.generations.insert(request)?;
        let state = state.clone();
        let variety = variety.to_string();
        let explanation = explanation.to_string();
        tauri::async_runtime::spawn(async move {
            let result = async {
                let _run = state
                    .generations
                    .claim_for(&request.id, "guide_translation")?;
                produce(
                    &state,
                    &request,
                    Translation {
                        edition: &edition,
                        variety: &variety,
                        explanation: &explanation,
                        key: &key,
                        prompt: &prompt,
                    },
                    &producer,
                )
                .await
            }
            .await;
            producer.finish(result);
        });
    }
    let mut result = decode(&subscription.wait().await?)?;
    result.context = Some(context);
    Ok(result)
}

fn decode(saved: &Retained) -> Result<SkillGuideResult> {
    let mut result: SkillGuideResult = serde_json::from_slice(&saved.payload).map_err(|_| {
        AppError::new(
            ErrorCode::Storage,
            "Saved guide translation has an invalid shape.",
        )
    })?;
    result.provenance["execution"] = json!(saved.execution);
    result.provenance["response"] = saved.metadata.clone();
    result.provenance["cacheHit"] = json!(saved.cached);
    Ok(result)
}

#[cfg(test)]
#[path = "../tests/skill_guides.rs"]
mod tests;

struct Translation<'a> {
    edition: &'a Edition,
    variety: &'a str,
    explanation: &'a str,
    key: &'a str,
    prompt: &'a str,
}
