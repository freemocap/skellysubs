//! Authored source selection and a translation contract that cannot rewrite examples.
use super::*;
use serde::Deserialize;
use serde_json::{Value, json};
use ts_rs::TS;

#[derive(Clone, Serialize)]
pub(crate) struct Edition {
    pub shared: authoring::Explanation,
    pub guide: authoring::Guide,
    pub paths: Vec<String>,
}

impl Registry {
    pub(crate) fn guide_edition(
        &self,
        language: &str,
        explanation: &str,
        skill: &str,
    ) -> Result<Option<Edition>> {
        self.language_config(language)?;
        self.language_config(explanation)?;
        self.skill_definition(skill)?;
        let stem = skill.replace('_', "-");
        let path = format!(
            "languages/{language}/skills/{stem}/{language}-{stem}-explained-in-{explanation}.yaml"
        );
        Ok(self.authored.guides.get(&path).map(|guide| Edition {
            shared: self.authored.explanations[&guide.shared_explanation].clone(),
            guide: guide.clone(),
            paths: vec![guide.shared_explanation.clone(), path],
        }))
    }

    pub(crate) fn guide_source(&self, language: &str, skill: &str) -> Result<Edition> {
        self.language_config(language)?;
        let explanation = &self.authored.guide_policy.source_explanation_languages[language];
        self.guide_edition(language, explanation, skill)?.ok_or_else(|| error(
            format!("languages/{language}/skills/{0}/{language}-{0}-explained-in-{explanation}.yaml", skill.replace('_', "-")),
            "missing_guide_source", "Required authored source guide is missing."))
    }

    pub(crate) fn guide_coach_prompt(&self) -> &str {
        &self.source_files["prompts/skills/coach-guide.md"]
    }

    pub(crate) fn guide_translation_prompt(&self) -> &str {
        &self.source_files["prompts/skills/guide-translation.md"]
    }
}

impl Edition {
    pub fn markdown(&self, variety: &str) -> String {
        super::communication_guides::render_guide(&self.shared, &self.guide, variety)
    }

    pub fn fingerprint(&self) -> String {
        fingerprint(self)
    }

    // Only explicit prose fields can be translated. Examples and IDs never come
    // back from the model, so source spelling/Unicode remains byte-for-byte intact.
    pub fn fields(&self, variety: &str) -> Vec<(String, String)> {
        let mut fields = vec![
            ("/shared/title".into(), self.shared.title.clone()),
            (
                "/shared/introduction".into(),
                self.shared.introduction.clone(),
            ),
        ];
        for (i, section) in self.shared.sections.iter().enumerate() {
            fields.push((format!("/shared/sections/{i}/title"), section.title.clone()));
            fields.push((
                format!("/shared/sections/{i}/concept"),
                section.concept.clone(),
            ));
        }
        let section_path = if self.guide.variety_sections.contains_key(variety) {
            format!("/guide/variety_sections/{variety}")
        } else {
            "/guide/sections".into()
        };
        for (i, section) in self.guide.sections_for(variety).iter().enumerate() {
            fields.push((
                format!("{section_path}/{i}/explanation"),
                section.explanation.clone(),
            ));
            for (j, example) in section.examples.iter().enumerate() {
                fields.push((
                    format!("{section_path}/{i}/examples/{j}/meaning"),
                    example.meaning.clone(),
                ));
                fields.push((
                    format!("{section_path}/{i}/examples/{j}/note"),
                    example.note.clone(),
                ));
            }
        }
        if let authoring::Disposition::Supplement { text, .. } = &self.guide.varieties[variety] {
            fields.push((format!("/guide/varieties/{variety}/text"), text.clone()));
        }
        fields
    }

    pub fn translated(&self, variety: &str, output: &str) -> model::Result<String> {
        let fields = self.fields(variety);
        let texts = decode_texts(output, fields.len())?;
        let invalid = invalid_translation;
        let mut value = json!({"shared":self.shared,"guide":self.guide});
        for ((path, _), text) in fields.iter().zip(texts) {
            *value.pointer_mut(path).ok_or_else(invalid)? = json!(text);
        }
        let shared = serde_json::from_value(value["shared"].clone()).map_err(|_| invalid())?;
        let guide = serde_json::from_value(value["guide"].clone()).map_err(|_| invalid())?;
        Ok(super::communication_guides::render_guide(
            &shared, &guide, variety,
        ))
    }
}

pub(crate) fn translation_schema(count: usize) -> Value {
    json!({"type":"object","additionalProperties":false,"properties":{"texts":{"type":"array","items":{"type":"string"},"minItems":count,"maxItems":count}},"required":["texts"]})
}

/// Shared provider-output validation for native execution and rendering.
pub(crate) fn decode_texts(output: &str, count: usize) -> model::Result<Vec<String>> {
    #[derive(Deserialize)]
    #[serde(deny_unknown_fields)]
    struct Translation {
        texts: Vec<String>,
    }
    let value: Translation = serde_json::from_str(output).map_err(|_| invalid_translation())?;
    if value.texts.len() != count
        || value
            .texts
            .iter()
            .any(|s| s.trim().is_empty() || s.len() > 16000 || s.contains('\0'))
    {
        return Err(invalid_translation());
    }
    Ok(value.texts)
}
fn invalid_translation() -> model::AppError {
    model::AppError::new(model::ErrorCode::Provider, "Guide translation failed validation.")
        .with_diagnostics(json!({"stage":"guide_translation_validation","path":"texts","expected":"one nonempty bounded string per source field"}))
}

#[derive(Clone, Debug, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct SkillGuideResult {
    pub markdown: String,
    pub explanation_language: String,
    pub generated: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub context: Option<GuideContext>,
    #[ts(type = "unknown")]
    pub provenance: Value,
}

#[derive(Clone, Debug, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct GuideReference {
    pub language: String,
    pub variety: String,
    pub skill_id: String,
    pub edition_language: String,
    pub fingerprint: String,
}

#[derive(Clone, Debug, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct GuideContext {
    pub reference: GuideReference,
    pub subskills: Vec<String>,
    pub examples: Vec<String>,
}

impl Edition {
    pub fn context(&self, variety: &str) -> GuideContext {
        GuideContext {
            reference: GuideReference {
                language: self.guide.language.clone(),
                variety: variety.into(),
                skill_id: self.guide.skill_id.clone(),
                edition_language: self.guide.explanation_language.clone(),
                fingerprint: self.fingerprint(),
            },
            subskills: self
                .guide
                .sections_for(variety)
                .iter()
                .map(|s| s.subskill_id.clone())
                .collect(),
            examples: self
                .guide
                .sections_for(variety)
                .iter()
                .flat_map(|s| s.examples.iter().map(|e| e.text.clone()))
                .collect(),
        }
    }
}

impl Registry {
    pub(crate) fn referenced_guide(&self, reference: &GuideReference) -> model::Result<Edition> {
        self.resolve_pair(
            &reference.language,
            Some(&reference.variety),
            &reference.edition_language,
            None,
        )?;
        let edition = self
            .guide_edition(
                &reference.language,
                &reference.edition_language,
                &reference.skill_id,
            )?
            .ok_or_else(|| {
                model::AppError::new(
                    model::ErrorCode::Conflict,
                    "The guide edition is unavailable. Reopen the guide.",
                )
            })?;
        if edition.fingerprint() != reference.fingerprint {
            return Err(model::AppError::new(
                model::ErrorCode::Conflict,
                "The guide changed. Reopen it before continuing.",
            ));
        }
        // Action indices refer to the visible variety, and focused coaching may
        // subsequently narrow these sections. Preserve source fingerprint checking above.
        let mut edition = edition;
        edition.guide.sections = edition.guide.sections_for(&reference.variety).to_vec();
        edition.guide.variety_sections.clear();
        Ok(edition)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn selected_variety_owns_rendering_translation_and_action_indices() {
        let registry = Registry::bundled().unwrap();
        for explanation in ["english", "cantonese"] {
            let edition = registry
                .guide_edition("arabic", explanation, "possibilities_constraints")
                .unwrap()
                .unwrap();
            for variety in ["arabic-modern-standard", "arabic-levantine"] {
                let sections = edition.guide.sections_for(variety);
                let expected: Vec<_> = sections
                    .iter()
                    .flat_map(|s| s.examples.iter().map(|e| e.text.clone()))
                    .collect();
                let context = edition.context(variety);
                assert_eq!(context.examples, expected);
                let resolved = registry.referenced_guide(&context.reference).unwrap();
                assert!(resolved.guide.variety_sections.is_empty());
                assert_eq!(resolved.context(variety).examples, expected);
                let texts: Vec<_> = edition
                    .fields(variety)
                    .into_iter()
                    .map(|(_, text)| text)
                    .collect();
                let translated = edition
                    .translated(variety, &json!({"texts": texts}).to_string())
                    .unwrap();
                assert_eq!(translated, edition.markdown(variety));
                for text in expected {
                    assert!(translated.contains(&format!("> {text}")));
                }
                let other = if variety == "arabic-levantine" {
                    "arabic-modern-standard"
                } else {
                    "arabic-levantine"
                };
                let other_example = &edition.guide.sections_for(other)[0].examples[0].text;
                assert!(!translated.contains(&format!("> {other_example}")));
                let mut stale = context.reference;
                stale.fingerprint.push_str("-stale");
                assert!(registry.referenced_guide(&stale).is_err());
            }
        }
    }

    #[test]
    fn translation_preserves_examples_and_rejects_partial_or_extra_output() {
        let mut registry = Registry::bundled().unwrap();
        let source = registry.guide_source("spanish", "time_events").unwrap();
        let texts: Vec<_> = source
            .fields("spanish-spain")
            .iter()
            .map(|(_, text)| format!("Translated: {text}"))
            .collect();
        let rendered = source
            .translated("spanish-spain", &json!({"texts":texts}).to_string())
            .unwrap();
        for section in &source.guide.sections {
            for example in &section.examples {
                assert!(rendered.contains(&format!("> {}", example.text)));
            }
        }
        assert!(rendered.contains("Translated:"));
        for bad in [
            json!({"texts":[]}),
            json!({"texts":texts,"examples":[]}),
            json!({"texts":[null]}),
        ] {
            assert!(
                source
                    .translated("spanish-spain", &bad.to_string())
                    .is_err()
            );
        }
        let mut changed = source.clone();
        changed.guide.revision.push_str("-changed");
        assert_ne!(source.fingerprint(), changed.fingerprint());
        changed = source.clone();
        changed.shared.introduction.push_str(" Changed.");
        assert_ne!(source.fingerprint(), changed.fingerprint());
        assert!(
            registry
                .guide_edition("spanish", "german", "time_events")
                .unwrap()
                .is_none()
        );
        let path =
            "languages/arabic/skills/time-events/arabic-time-events-explained-in-english.yaml";
        assert!(registry.authored.guides.remove(path).is_some());
        assert!(registry.guide_source("arabic", "time_events").is_err());
    }
}

/// The exact authored material selected for a coach question.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
pub enum GuideCoachFocus {
    Subskill { subskill_id: String },
    Example { index: usize },
}
