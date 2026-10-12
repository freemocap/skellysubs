//! Shared source-bound gloss validation and projection for all consumers.
use crate::ai::transport::provider::Completion;
use crate::ai::transport::provider::RequestOutput;
use crate::language::linguistics;
use crate::language::linguistics::Annotation;
use crate::language::linguistics::Coverage;
use crate::language::linguistics::SourceIdentity;
use crate::language::linguistics::SourceMap;
use crate::language::linguistics::adapter;
use crate::model::AppError;
use crate::model::ErrorCode;
use crate::model::GlossCoverage;
use crate::model::GlossSegment;
use crate::model::GlossSegmentKind;
use crate::model::Result;
use crate::model::WordGlossView;

/// Model role for word meanings, in conversation turns and explicit reading requests.
pub(crate) const ROLE: &str = "standard";

#[derive(Debug, Clone)]
pub struct Source {
    pub identity: SourceIdentity,
    pub text: String,
}

pub fn request_output<'a>(
    source: Option<&Source>,
    schema: &'a serde_json::Value,
) -> RequestOutput<'a> {
    if source.is_some() {
        RequestOutput::JsonSchema {
            max_output_tokens: crate::ai::transport::provider::GLOSS_OUTPUT_TOKENS,
            name: adapter::FORMAT_ID,
            schema,
        }
    } else {
        RequestOutput::Prose
    }
}

pub fn validation_error() -> AppError {
    AppError::new(
        ErrorCode::Provider,
        "Word meanings failed the source-bound output contract. No new word meanings were saved.",
    )
}

pub fn validate(
    source: &Source,
    completion: &Completion,
    operation: &str,
    attempt: &str,
) -> Result<WordGlossView> {
    let analysis =
        adapter::validate_word_gloss_completion(&source.identity, &source.text, completion)
            .map_err(|error| gloss_error(error, completion))?;
    project(source, analysis, operation, attempt)
}
pub fn recover_with_context(
    source: &Source,
    completion: &Completion,
    operation: &str,
    attempt: &str,
    context: &crate::configuration::LanguageContext,
) -> Result<(WordGlossView, serde_json::Value)> {
    let recovered = adapter::recovery::recover(&source.identity, &source.text, completion, context)
        .map_err(|error| gloss_error(error, completion))?;
    let report = serde_json::json!({"policy": "word-gloss-recovery-v1", "rejected_spans": recovered.rejected});
    Ok((
        project(source, recovered.analysis, operation, attempt)?,
        report,
    ))
}

/// Repair can fill only unresolved intervals; accepted annotations never move.
pub fn merge_repair(previous: &WordGlossView, mut next: WordGlossView) -> Result<WordGlossView> {
    if previous.source_message_id != next.source_message_id
        || previous.target_language_id != next.target_language_id
        || previous.explanation_language_id != next.explanation_language_id
        || previous.boundary_policy != next.boundary_policy
    {
        return Err(validation_error());
    }
    let mut segments = Vec::new();
    for old in &previous.segments {
        if old.kind != GlossSegmentKind::Unresolved {
            segments.push(old.clone());
            continue;
        }
        let mut cursor = old.start;
        for candidate in next.segments.iter().filter(|s| {
            s.start >= old.start && s.end <= old.end && s.kind != GlossSegmentKind::Unresolved
        }) {
            if candidate.start > cursor {
                let mut gap = old.clone();
                gap.start = cursor;
                gap.end = candidate.start;
                segments.push(gap);
            }
            segments.push(candidate.clone());
            cursor = candidate.end;
        }
        if cursor < old.end {
            let mut gap = old.clone();
            gap.start = cursor;
            segments.push(gap);
        }
    }
    next.coverage = if segments
        .iter()
        .any(|s| s.kind == GlossSegmentKind::Unresolved)
    {
        GlossCoverage::Partial
    } else {
        GlossCoverage::Complete
    };
    next.template_version = "persona-word-gloss-repair-v1".into();
    next.segments = segments;
    Ok(next)
}

fn gloss_error(error: adapter::AdapterError, completion: &Completion) -> AppError {
    if matches!(error, adapter::AdapterError::InvalidTermination)
        && completion.finish_reason == "length"
    {
        return AppError::new(
            ErrorCode::Provider,
            "Word meanings reached the provider's output limit (gloss_invalid_termination). No new word meanings were saved. Retry word meanings.",
        );
    }
    let location = error
        .span_index()
        .map(|index| format!("; span {index}"))
        .unwrap_or_default();
    AppError::new(
        ErrorCode::Provider,
        format!(
            "Word meanings rejected: {}{}. No new word meanings were saved.",
            error.diagnostic_code(),
            location
        ),
    )
    .with_diagnostics(error.diagnostics())
}
pub(super) fn project(
    source: &Source,
    analysis: linguistics::ValidatedAnalysis,
    operation: &str,
    attempt: &str,
) -> Result<WordGlossView> {
    let map = SourceMap::new(&source.text).map_err(|_| validation_error())?;
    let segments = analysis
        .segments()
        .iter()
        .map(|segment| {
            let span = map
                .utf16_span(segment.span)
                .map_err(|_| validation_error())?;
            let (kind, gloss) = match &segment.annotation {
                Annotation::Gloss { gloss, .. } => (GlossSegmentKind::Gloss, Some(gloss.clone())),
                Annotation::Literal => (GlossSegmentKind::Literal, None),
                Annotation::Unresolved => (GlossSegmentKind::Unresolved, None),
            };
            Ok(GlossSegment {
                start: span.start as u32,
                end: span.end as u32,
                kind,
                gloss,
                romanization: match &segment.annotation {
                    Annotation::Gloss { romanization, .. } => romanization.clone(),
                    _ => None,
                },
                pronunciation: match &segment.annotation {
                    Annotation::Gloss { pronunciation, .. } => pronunciation.clone(),
                    _ => None,
                },
            })
        })
        .collect::<Result<Vec<_>>>()?;
    Ok(WordGlossView {
        source_message_id: source.identity.message_id.clone(),
        target_language_id: source.identity.target_language_id.clone(),
        explanation_language_id: source.identity.explanation_language_id.clone(),
        format_version: adapter::FORMAT_ID.into(),
        template_version: adapter::TEMPLATE_ID.into(),
        boundary_policy: linguistics::BOUNDARY_POLICY.into(),
        operation_id: operation.into(),
        attempt_id: attempt.into(),
        coverage: match analysis.coverage() {
            Coverage::Complete => GlossCoverage::Complete,
            Coverage::Partial => GlossCoverage::Partial,
        },
        segments,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    fn source() -> Source {
        Source {
            identity: SourceIdentity {
                message_id: "message".into(),
                target_language_id: "spanish".into(),
                explanation_language_id: "english".into(),
                analysis_version: linguistics::ANALYSIS_VERSION.into(),
            },
            text: "𐐀 sí sí!".into(),
        }
    }
    fn completion() -> Completion {
        Completion { diagnostics: None, text: r#"{"spans":[{"first":"g0002","last":"g0003","kind":"gloss","gloss":"yes"},{"first":"g0005","last":"g0006","kind":"gloss","gloss":"indeed"}]}"#.into(), finish_reason: "stop".into(), actual_model: "fixture".into(), provider_id: "fixture".into(), input_tokens: Some(10), output_tokens: Some(4) }
    }
    #[test]
    fn projection_preserves_repeated_occurrences_and_utf16_source_binding() {
        let source = source();
        let view = validate(&source, &completion(), "operation", "attempt").unwrap();
        let words: Vec<_> = view
            .segments
            .iter()
            .filter(|s| s.kind == GlossSegmentKind::Gloss)
            .collect();
        assert_eq!((words[0].start, words[0].end), (3, 5));
        assert_eq!((words[1].start, words[1].end), (6, 8));
        assert_eq!(words[0].gloss.as_deref(), Some("yes"));
        assert_eq!(words[1].gloss.as_deref(), Some("indeed"));
        assert_eq!(view.coverage, GlossCoverage::Partial);
        assert_eq!(view.source_message_id, "message");
        assert_eq!(view.operation_id, "operation");
        assert_eq!(view.attempt_id, "attempt");
        assert_eq!(
            serde_json::from_str::<WordGlossView>(&serde_json::to_string(&view).unwrap()).unwrap(),
            view
        );
    }
    #[test]
    fn projection_preserves_readings_for_the_exact_source_span() {
        let mut source = source();
        source.text = "你好".into();
        source.identity.target_language_id = "mandarin".into();
        let mut output = completion();
        output.text = r#"{"spans":[{"first":"g0000","last":"g0001","kind":"gloss","gloss":"hello","romanization":"nǐ hǎo","pronunciation":null}]}"#.into();
        let view = validate(&source, &output, "operation", "attempt").unwrap();
        assert_eq!(view.segments[0].romanization.as_deref(), Some("nǐ hǎo"));
        assert_eq!(view.segments[0].pronunciation, None);
        assert_eq!((view.segments[0].start, view.segments[0].end), (0, 2));
    }
    #[test]
    fn projection_rejects_bad_output_without_echoing_content_or_consuming_usage() {
        let mut output = completion();
        output.text = "private malformed response".into();
        let error = validate(&source(), &output, "operation", "attempt").unwrap_err();
        assert!(!error.message.contains("private"));
        assert!(error.message.starts_with("Word meanings rejected: "));
        assert_eq!(output.input_tokens, Some(10));
        assert_eq!(output.output_tokens, Some(4));
    }
    #[test]
    fn invalid_gloss_json_preserves_usage_independent_of_finish_label() {
        let mut output = completion();
        output.finish_reason = "length".into();
        output.text = "private incomplete response".into();
        let error = validate(&source(), &output, "operation", "attempt").unwrap_err();
        assert!(error.message.contains("gloss_invalid_json_or_shape"));
        assert!(!error.message.contains("private"));
        assert_eq!(output.input_tokens, Some(10));
        assert_eq!(output.output_tokens, Some(4));
    }
}
