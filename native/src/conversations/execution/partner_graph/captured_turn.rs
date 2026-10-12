//! Admission projection from finalized application capture to typed graph inputs.
//! Inputs carry the captured source and settings; refinement is local and deterministic.
use super::*;
use crate::{ai::connections::model_routing, language::source_graph::SourceText};
use serde::de::DeserializeOwned;
use serde_json::Value;

fn read<T: DeserializeOwned>(value: &Value, field: &str) -> Result<T> {
    serde_json::from_value(value[field].clone()).map_err(|_| Fault {
        code: "partner_turn_capture_invalid".into(),
        path: field.into(),
    })
}
// Recording identities remain in the accepted turn's source provenance. The
// version-1 reasoning context consumes modality/help flags, not host record IDs.
fn reasoning_input(captured: &Value) -> Result<crate::learning::coaching::InputEvidence> {
    let mut input: crate::learning::coaching::InputEvidence = read(captured, "input")?;
    input.recording_ids = None;
    Ok(input)
}
pub fn inputs(
    kind: context::Kind,
    captured: &Value,
    learner_source: Option<SourceText>,
    reply_source_id: String,
    install_id: String,
) -> Result<Values> {
    let language: crate::configuration::LanguageContext = read(captured, "languageContext")?;
    let messages =
        read::<Vec<crate::ai::transport::provider::PromptMessage>>(captured, "messages")?;
    let standard: ResolvedTarget = read(captured, "target")?;
    let fast_model: String = read(captured, "fastModel")?;
    let target = |role| model_routing::target(&standard, role, &fast_model);
    let skill_values = match captured["presenceSkills"].as_array() {
        Some(skills) => Some(skills),
        None if captured["presenceSkills"].is_null()
            && !captured["presenceContentError"].is_null() =>
        {
            None
        }
        None => {
            return Err(Fault {
                code: "partner_turn_capture_invalid".into(),
                path: "presenceSkills".into(),
            });
        }
    };
    let skills = skill_values
        .map(|skills| {
            skills
                .iter()
                .map(|skill| {
                    Ok(support_graph::explanation::Skill {
                        id: read(skill, "id")?,
                        name: read(skill, "name")?,
                        overview: read(skill, "overview")?,
                    })
                })
                .collect::<Result<Vec<_>>>()
        })
        .transpose()?;
    let support = support::Captured {
        context: support_graph::Context {
            messages: messages.clone(),
            target_language: read(captured, "targetLanguage")?,
            translation_language: read(captured, "translationLanguage")?,
            language_context: support_graph::Language {
                script: language.script.clone(),
                guidance: language.guidance.clone(),
            },
            practice_settings: support_graph::Practice {
                difficulty: read(&captured["practiceSettings"], "difficulty")?,
            },
            input: if matches!(kind, context::Kind::Reply) {
                Some(reasoning_input(captured)?)
            } else {
                None
            },
        },
        skills,
        brief_target: target("fast"),
        assistance_target: target("standard"),
    };
    let (assessment, feedback) = if matches!(kind, context::Kind::Reply) {
        let assessment = AssessmentCapture {
            context: assessment_graph::Context {
                messages: messages.clone(),
                input: reasoning_input(captured)?,
                language: language.target_name.clone(),
                variety: language.variety_name.clone(),
            },
            content: skill_values
                .map(|_| -> Result<_> {
                    Ok(assessment_graph::Content {
                        skills: read(captured, "presenceSkills")?,
                        instructions: read(captured, "presenceInstructions")?,
                        questions: read(captured, "messageAssessmentQuestions")?,
                    })
                })
                .transpose()?,
            target: target("fast"),
        };
        let candidates = captured["candidateConstructs"]
            .as_array()
            .ok_or_else(|| Fault {
                code: "partner_turn_capture_invalid".into(),
                path: "candidateConstructs".into(),
            })?
            .iter()
            .map(|candidate| {
                Ok(feedback_graph::Candidate {
                    id: read(candidate, "id")?,
                    criterion: read(candidate, "criterion")?,
                })
            })
            .collect::<Result<Vec<_>>>()?;
        let feedback = feedback::Captured {
            context: feedback_graph::Context {
                messages: messages.clone(),
                input: reasoning_input(captured)?,
                target_language: read(captured, "targetLanguage")?,
                translation_language: read(captured, "translationLanguage")?,
                candidate_constructs: candidates,
                practice_settings: feedback_graph::Practice {
                    difficulty: read(&captured["practiceSettings"], "difficulty")?,
                    coach_proactivity: read(&captured["practiceSettings"], "coachProactivity")?,
                },
                practice_focus: if captured["practiceFocus"].is_null() {
                    None
                } else {
                    Some(feedback_graph::Focus {
                        id: read(&captured["practiceFocus"], "id")?,
                    })
                },
                feedback_context: read(captured, "feedbackContext")?,
                feedback_policy: read(captured, "feedbackPolicy")?,
                guidance: language.guidance.clone(),
            },
            target: target("standard"),
        };
        (Some(assessment), Some(feedback))
    } else {
        (None, None)
    };
    let speech = if captured["speechTarget"].is_null() {
        None
    } else {
        Some(crate::speech::synthesis_graph::capture_settings(
            crate::speech::synthesis_graph::Settings {
                target: read(captured, "speechTarget")?,
                install_id: install_id.clone(),
                language_tag: language
                    .external_tags
                    .get("language_tag")
                    .cloned()
                    .ok_or_else(|| Fault {
                        code: "partner_turn_capture_invalid".into(),
                        path: "languageContext.external_tags.language_tag".into(),
                    })?,
                language: format!("{} — {}", language.target_name, language.variety_name),
                voice: read(captured, "speechVoice")?,
            },
        )?)
    };
    let mut values = capture(
        reply_reading_graph::Captured {
            context: context::Captured {
                kind,
                messages,
                source_ids: read(captured, "sourceIds")?,
            },
            learner_source,
            reply_source_id,
            languages: translation_graph::Languages {
                source: read(captured, "targetLanguage")?,
                destination: read(captured, "translationLanguage")?,
                destination_writing: language.guidance("explanation_writing"),
            },
            gloss_settings: (&language).into(),
            gloss_target: target(crate::language::gloss::ROLE),
            reply_target: target("fast"),
            reading_target: target(crate::language::translation::ROLE),
            install_id,
        },
        assessment,
        support,
        feedback,
    )?;
    if let Some(speech) = speech {
        values.insert("speech".into(), speech);
    }
    Ok(values)
}

/// Policy changes activation only; the executable always contains these nodes.
pub fn policy(kind: context::Kind, captured: &Value) -> Result<BTreeMap<String, Activation>> {
    use crate::configuration::execution::{ExecutionMode, ExecutionPreferences};
    let preferences: ExecutionPreferences = read(captured, "executionPreferences")?;
    let mode = |mode| match mode {
        ExecutionMode::Automatic => Activation::Automatic,
        ExecutionMode::OnDemand => Activation::OnDemand,
    };
    let mut policy = BTreeMap::from([
        (
            "speech/lookup".into(),
            if read::<bool>(captured, "speechEnabled")? {
                Activation::Automatic
            } else {
                Activation::OnDemand
            },
        ),
        ("brief".into(), mode(preferences.reply_brief)),
        ("reply_translation".into(), mode(preferences.reading)),
        ("reply_gloss".into(), mode(preferences.reading)),
    ]);
    if matches!(kind, context::Kind::Reply) {
        policy.extend([
            ("assessment".into(), mode(preferences.assessment)),
            // Selection and attribution follow an explicitly demanded assessment.
            ("learner_translation".into(), mode(preferences.reading)),
            ("learner_gloss".into(), mode(preferences.reading)),
        ]);
    }
    Ok(policy)
}
