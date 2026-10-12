//! Captured semantic inputs for gloss prompting and validation. Transport,
//! presentation and unrelated language configuration are not computation inputs.
use super::*;

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Settings {
    pub target_language: String,
    pub explanation_language: String,
    pub explanation_writing: Vec<String>,
    pub romanization: Vec<String>,
    pub segmentation: Vec<String>,
    pub romanization_enabled: bool,
}

impl From<&crate::configuration::LanguageContext> for Settings {
    fn from(context: &crate::configuration::LanguageContext) -> Self {
        let romanization = context.guidance("romanization");
        Self {
            target_language: context.language_id.clone(),
            explanation_language: context.explanation_language_id.clone(),
            explanation_writing: context.guidance("explanation_writing"),
            romanization_enabled: context.script != "latin" && !romanization.is_empty(),
            romanization,
            segmentation: context.guidance("segmentation"),
        }
    }
}

impl Settings {
    pub fn prompt(
        &self,
        identity: &SourceIdentity,
        source: &str,
    ) -> Result<GlossPrompt, AdapterError> {
        build_word_gloss_prompt_context(identity, source, Some(self))
    }
}
