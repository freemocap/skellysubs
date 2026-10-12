//! Recover independently valid annotations without guessing source locations.
use super::*;
use crate::language::linguistics::{Origin, Span};
use unicode_segmentation::UnicodeSegmentation;

#[derive(Debug, Serialize)]
pub struct RejectedSpan {
    pub index: usize,
    pub code: &'static str,
    pub diagnostics: serde_json::Value,
}

pub struct Recovered {
    pub analysis: ValidatedAnalysis,
    pub rejected: Vec<RejectedSpan>,
}

// Preserve the strict decoder's duplicate-field rejection before converting a
// row to Value for independent validation. Never collapse ambiguous anchors.
struct UniqueRow(serde_json::Value);
impl<'de> Deserialize<'de> for UniqueRow {
    fn deserialize<D: Deserializer<'de>>(decoder: D) -> Result<Self, D::Error> {
        struct RowVisitor;
        impl<'de> Visitor<'de> for RowVisitor {
            type Value = UniqueRow;
            fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_str("an annotation object without duplicate fields")
            }
            fn visit_map<M: serde::de::MapAccess<'de>>(
                self,
                mut map: M,
            ) -> Result<UniqueRow, M::Error> {
                let mut fields = serde_json::Map::new();
                while let Some(key) = map.next_key::<String>()? {
                    if fields.contains_key(&key) {
                        return Err(de::Error::custom("duplicate annotation field"));
                    }
                    fields.insert(key, map.next_value()?);
                }
                Ok(UniqueRow(serde_json::Value::Object(fields)))
            }
        }
        decoder.deserialize_map(RowVisitor)
    }
}

/// Envelope, source identity and completion termination remain strict. Individual
/// annotations can fail independently; overlapping annotations are all rejected.
pub fn recover(
    identity: &SourceIdentity,
    source: &str,
    completion: &provider::Completion,
    context: &crate::configuration::LanguageContext,
) -> Result<Recovered, AdapterError> {
    recover_with_settings(
        identity,
        source,
        completion,
        &settings::Settings::from(context),
    )
}

pub fn recover_with_settings(
    identity: &SourceIdentity,
    source: &str,
    completion: &provider::Completion,
    context: &settings::Settings,
) -> Result<Recovered, AdapterError> {
    if completion.finish_reason == "error" {
        return Err(AdapterError::InvalidTermination);
    }
    if completion.text.len() > MAX_RESPONSE_BYTES {
        return Err(AdapterError::PayloadTooLarge);
    }
    source_map(identity, source, Some(context))?;
    #[derive(Deserialize)]
    struct Envelope {
        spans: Vec<UniqueRow>,
    }
    let wire: Envelope = serde_json::from_str(&completion.text).map_err(AdapterError::json)?;
    if wire.spans.len() > MAX_SPANS {
        return Err(AdapterError::InvalidCandidate(
            ValidationError::TooManySpans,
        ));
    }
    let mut accepted = Vec::new();
    let mut rejected = Vec::new();
    for (index, item) in wire.spans.into_iter().enumerate() {
        let raw = serde_json::json!({"spans":[item.0]}).to_string();
        match decode_word_gloss_context(identity, source, &raw, Some(context)) {
            Ok(value) => {
                for segment in value
                    .segments()
                    .iter()
                    .filter(|s| s.origin == Origin::Candidate)
                {
                    accepted.push((
                        index,
                        CandidateSpan {
                            span: segment.span,
                            annotation: segment.annotation.clone(),
                        },
                    ));
                }
            }
            Err(error) => rejected.push(RejectedSpan {
                index,
                code: error.diagnostic_code(),
                diagnostics: error.diagnostics(),
            }),
        }
    }
    let conflicts: Vec<_> = accepted
        .iter()
        .map(|(index, item)| {
            accepted.iter().any(|(other, candidate)| {
                index != other
                    && item.span.start < candidate.span.end
                    && candidate.span.start < item.span.end
            })
        })
        .collect();
    let mut spans = Vec::new();
    for ((index, item), conflict) in accepted.into_iter().zip(conflicts) {
        if conflict {
            rejected.push(RejectedSpan {
                index,
                code: "gloss_overlap_or_unordered",
                diagnostics: serde_json::json!({"stage":"word_gloss_validation","reason":"overlapping_annotations","index":index}),
            });
        } else {
            spans.push(item);
        }
    }
    // Fill only unannotated, unambiguous nonlexical graphemes. Apostrophes,
    // hyphens, digits, currency/math symbols and emoji stay unresolved.
    let mut start = 0;
    for grapheme in source.graphemes(true) {
        let end = start + grapheme.chars().count();
        if spans.len() < MAX_SPANS
            && grapheme
                .chars()
                .all(|c| c.is_whitespace() || ".!?¿¡。！？،؛؟,:;()[]{}«»“”".contains(c))
            && !spans
                .iter()
                .any(|s| s.span.start < end && start < s.span.end)
        {
            spans.push(CandidateSpan {
                span: Span { start, end },
                annotation: Annotation::Literal,
            });
        }
        start = end;
    }
    spans.sort_by_key(|s| s.span.start);
    // Coalesce only adjacent deterministic/literal spans to retain the core bound.
    let mut compact: Vec<CandidateSpan> = Vec::new();
    for item in spans {
        if let Some(previous) = compact.last_mut()
            && previous.span.end == item.span.start
            && previous.annotation == Annotation::Literal
            && item.annotation == Annotation::Literal
        {
            previous.span.end = item.span.end;
            continue;
        }
        compact.push(item);
    }
    let analysis = super::super::validate(
        identity,
        source,
        &Candidate {
            source: identity.clone(),
            spans: compact,
        },
    )
    .map_err(AdapterError::InvalidCandidate)?;
    Ok(Recovered { analysis, rejected })
}

/// Translate saved UI coordinates back to exact provider row IDs locally.
pub fn repair_prompt(
    mut prompt: GlossPrompt,
    source: &str,
    gaps: &[(u32, u32)],
) -> Result<GlossPrompt, AdapterError> {
    let map = SourceMap::new(source).map_err(AdapterError::InvalidSource)?;
    let rows = grapheme_rows(&map);
    let mut targets = Vec::new();
    for (start, end) in gaps {
        let selected: Vec<_> = rows
            .iter()
            .filter(|r| {
                map.utf16_span(r.span)
                    .is_ok_and(|s| s.start >= *start as usize && s.end <= *end as usize)
            })
            .collect();
        if let (Some(first), Some(last)) = (selected.first(), selected.last()) {
            targets.push(serde_json::json!({"first":first.id,"last":last.id}));
        }
    }
    prompt.messages.push(PromptMessage { role: "user".into(), content: format!("Repair only missing word meanings within these inclusive source grapheme ranges: {}. Leave other regions out; previously accepted meanings are retained. Use exact row IDs from the original source table. Do not rewrite the source or return annotations crossing these ranges.", serde_json::to_string(&targets).map_err(|_| AdapterError::Serialization)?) });
    finish_prompt(prompt.messages, prompt.output_schema)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn run(source: &str, spans: serde_json::Value) -> Recovered {
        let registry = crate::configuration::Registry::bundled().unwrap();
        let context = registry
            .resolve_pair("spanish", None, "english", None)
            .unwrap();
        let identity = SourceIdentity {
            message_id: "m".into(),
            target_language_id: "spanish".into(),
            explanation_language_id: "english".into(),
            analysis_version: super::super::super::ANALYSIS_VERSION.into(),
        };
        let completion = provider::Completion {
            text: serde_json::json!({"spans":spans}).to_string(),
            finish_reason: "stop".into(),
            actual_model: "test".into(),
            provider_id: "test".into(),
            input_tokens: None,
            output_tokens: None,
            diagnostics: None,
        };
        recover(&identity, source, &completion, &context).unwrap()
    }
    fn word(first: &str, last: &str) -> serde_json::Value {
        serde_json::json!({"first":first,"last":last,"kind":"gloss","gloss":"meaning"})
    }
    #[test]
    fn omitted_punctuation_is_complete_but_lexical_gaps_are_not() {
        let complete = run("¿Hola?", serde_json::json!([word("g0001", "g0004")]));
        assert_eq!(
            complete.analysis.coverage(),
            super::super::super::Coverage::Complete
        );
        let missing = run("Hola, tú?", serde_json::json!([word("g0000", "g0003")]));
        assert_eq!(missing.analysis.unresolved_scalar_count(), 2);
        assert_eq!(missing.analysis.gloss_count(), 1);
        for source in ["42", "€", "😀", "-", "'", "क्"] {
            assert_eq!(
                run(source, serde_json::json!([])).analysis.coverage(),
                super::super::super::Coverage::Partial
            );
        }
    }
    #[test]
    fn bad_whitespace_and_unknown_anchor_do_not_discard_valid_words() {
        let result = run(
            "Hola tú",
            serde_json::json!([
                word("g0000", "g0003"),
                word("g0004", "g0004"),
                word("bogus", "g0006")
            ]),
        );
        assert_eq!(result.analysis.gloss_count(), 1);
        assert_eq!(result.analysis.unresolved_scalar_count(), 2);
        assert_eq!(result.rejected.len(), 2);
        assert_eq!(result.rejected[0].code, "gloss_whitespace_target");
    }
    #[test]
    fn conflicting_spans_are_both_removed_and_disjoint_spans_can_be_sorted() {
        let result = run(
            "Hola tú",
            serde_json::json!([
                word("g0005", "g0006"),
                word("g0000", "g0003"),
                word("g0000", "g0001")
            ]),
        );
        assert_eq!(result.analysis.gloss_count(), 1);
        assert_eq!(result.rejected.len(), 2);
        assert_eq!(result.analysis.gloss_scalar_count(), 2);
    }
    #[test]
    fn recovery_preserves_combining_marks_and_repairs_use_scalar_row_ids() {
        let result = run(
            "نُور؟",
            serde_json::json!([word("g0000", "g0003"), word("g0001", "g0001")]),
        );
        assert_eq!(
            result.analysis.coverage(),
            super::super::super::Coverage::Complete
        );
        assert_eq!(result.analysis.gloss_scalar_count(), 4);
        assert_eq!(result.rejected.len(), 1);
        let identity = SourceIdentity {
            message_id: "m".into(),
            target_language_id: "spanish".into(),
            explanation_language_id: "english".into(),
            analysis_version: super::super::super::ANALYSIS_VERSION.into(),
        };
        let prompt = build_word_gloss_prompt(&identity, "😀 tú").unwrap();
        let prompt = repair_prompt(prompt, "😀 tú", &[(3, 5)]).unwrap();
        let instruction = &prompt.messages.last().unwrap().content;
        assert!(instruction.contains("g0002"));
        assert!(instruction.contains("g0003"));
        assert!(!instruction.contains("g0005"));
    }
    #[test]
    fn recovery_does_not_collapse_duplicate_source_anchors() {
        assert!(
            serde_json::from_str::<UniqueRow>(
                r#"{"first":"g0000","first":"g0002","last":"g0003","kind":"gloss","gloss":"x"}"#
            )
            .is_err()
        );
    }
}
