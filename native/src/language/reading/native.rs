//! Reading request capture and projection reuse the same typed operations as
//! conversation nodes. No raw provider response is reparsed during publication.
use super::*;
use crate::ai::graph::Values;
use crate::conversations::execution::graph_runtime::error;
use crate::language::{gloss_graph, source_graph::SourceText, translation_graph};

impl Request {
    pub(crate) fn native_inputs(&self) -> Result<Values> {
        let source = SourceText {
            id: self.source().identity.message_id,
            text: self.input.text.clone(),
        };
        let mut target = self.target.clone();
        target.model = self.model.clone();
        match self.input.aid {
            ReadingAid::Translation => translation_graph::capture(
                source,
                translation_graph::Languages {
                    source: self.context.language_id.clone(),
                    destination: self.context.explanation_language_id.clone(),
                    destination_writing: self.context.guidance("explanation_writing"),
                },
                &target,
                &self.install,
            ),
            ReadingAid::Explanations | ReadingAid::Completions => explanation_graph::capture(
                source,
                explanation_graph::Context {
                    target: self.context.language_id.clone(),
                    explanation: self.context.explanation_language_id.clone(),
                    script: self.context.script.clone(),
                    guidance: self.context.guidance.clone(),
                    template: self.input.aid == ReadingAid::Completions,
                },
                &target,
                &self.install,
            ),
            ReadingAid::WordGloss => {
                gloss_graph::capture(source, (&self.context).into(), &target, &self.install)
            }
            _ => {
                return Err(AppError::new(
                    ErrorCode::Validation,
                    "Unsupported native reading aid.",
                ));
            }
        }
        .map_err(error)
    }
    pub(crate) fn native_context(&self) -> Result<serde_json::Value> {
        Ok(
            serde_json::json!({"scope":ReadingScope {language:self.context.language_id.clone(),variety:Some(self.context.variety_id.clone()),explanation:self.context.explanation_language_id.clone(),explanation_variety:Some(self.context.explanation_variety_id.clone())},"accessScope":self.text_scope()?,"sourceKey":crate::ai::results::digest(&serde_json::to_vec(&serde_json::json!([self.input.text,self.context,self.input.aid,self.text_scope()?]))?)}),
        )
    }
}

pub(crate) fn project(
    values: &Values,
    context: &serde_json::Value,
    engine: &str,
    run: &str,
    node: &str,
    attempt: crate::ai::graph::AttemptId,
) -> Result<text::Stored> {
    let operation = format!("graph:{}", serde_json::to_string(&(engine, run, node))?);
    let attempt = format!("graph:{}", serde_json::to_string(&(engine, attempt))?);
    let mut explanations = None;
    let (text, translation, gloss) = if let Some(value) = values.get("gloss") {
        let analysis: gloss_graph::result::Analysis = serde_json::from_value(value.clone())?;
        (
            analysis.source.text.clone(),
            None,
            Some(analysis.view(&operation, &attempt).map_err(error)?),
        )
    } else if let Some(value) = values.get("explanations") {
        let source: SourceText = serde_json::from_value(value["source"].clone())?;
        explanations = Some(serde_json::from_value(value["value"].clone())?);
        (source.text, None, None)
    } else if let Some(value) = values.get("translation") {
        let source: SourceText = serde_json::from_value(value["source"].clone())?;
        (
            source.text,
            Some(
                value["text"]
                    .as_str()
                    .ok_or_else(|| {
                        AppError::new(ErrorCode::Storage, "Native translation has no text.")
                    })?
                    .to_owned(),
            ),
            None,
        )
    } else {
        return Err(AppError::new(
            ErrorCode::Storage,
            "Native reading result is missing.",
        ));
    };
    Ok(text::Stored {
        scope: serde_json::from_value(context["scope"].clone())?,
        text,
        access_scope: context["accessScope"].as_str().unwrap_or_default().into(),
        gloss,
        translation,
        explanations,
        completion: None,
    })
}
