//! Typed source annotations use scalar coordinates. Reading annotations are
//! separate optional relations, preserving absence without fabricated strings.
use super::*;
use crate::language::linguistics::{self, Annotation, Candidate, CandidateSpan, Span, Unit};

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Range {
    pub start: usize,
    pub end: usize,
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct TextSpan {
    pub start: usize,
    pub end: usize,
    pub text: String,
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Analysis {
    pub source: SourceText,
    pub target_language: String,
    pub explanation_language: String,
    pub words: Vec<TextSpan>,
    pub literals: Vec<Range>,
    pub romanizations: Vec<TextSpan>,
    pub pronunciations: Vec<TextSpan>,
}

impl Analysis {
    pub(super) fn capture(source: SourceText, analysis: &linguistics::ValidatedAnalysis) -> Self {
        let mut result = Self {
            source,
            target_language: analysis.source().target_language_id.clone(),
            explanation_language: analysis.source().explanation_language_id.clone(),
            words: vec![],
            literals: vec![],
            romanizations: vec![],
            pronunciations: vec![],
        };
        for segment in analysis.segments() {
            let text = |text: &str| TextSpan {
                start: segment.span.start,
                end: segment.span.end,
                text: text.into(),
            };
            match &segment.annotation {
                Annotation::Gloss {
                    gloss,
                    romanization,
                    pronunciation,
                    ..
                } => {
                    result.words.push(text(gloss));
                    if let Some(value) = romanization {
                        result.romanizations.push(text(value));
                    }
                    if let Some(value) = pronunciation {
                        result.pronunciations.push(text(value));
                    }
                }
                Annotation::Literal => result.literals.push(Range {
                    start: segment.span.start,
                    end: segment.span.end,
                }),
                // Missing annotations remain unresolved under the shared validator.
                Annotation::Unresolved => (),
            }
        }
        result
    }

    /// Revalidate persisted values before projection. Coordinates and missing
    /// annotations retain the existing linguistics policy; no viewer inference.
    pub fn validate(&self) -> graph::Result<linguistics::ValidatedAnalysis> {
        let identity = identity(
            &self.source,
            &self.target_language,
            &self.explanation_language,
        );
        let mut spans = Vec::new();
        let reading = |readings: &[TextSpan], word: &TextSpan| -> graph::Result<Option<String>> {
            let mut found = readings
                .iter()
                .filter(|r| r.start == word.start && r.end == word.end);
            let value = found.next().map(|r| r.text.clone());
            if found.next().is_some() {
                return Err(fault("duplicate_reading"));
            }
            Ok(value)
        };
        for item in self.romanizations.iter().chain(&self.pronunciations) {
            if !self
                .words
                .iter()
                .any(|word| word.start == item.start && word.end == item.end)
            {
                return Err(fault("unbound_reading"));
            }
            provider::validate_prose(&item.text).map_err(|_| fault("reading"))?;
        }
        for word in &self.words {
            provider::validate_prose(&word.text).map_err(|_| fault("word"))?;
            spans.push(CandidateSpan {
                span: Span {
                    start: word.start,
                    end: word.end,
                },
                annotation: Annotation::Gloss {
                    unit: Unit::Word,
                    gloss: word.text.clone(),
                    romanization: reading(&self.romanizations, word)?,
                    pronunciation: reading(&self.pronunciations, word)?,
                },
            });
        }
        spans.extend(self.literals.iter().map(|range| CandidateSpan {
            span: Span {
                start: range.start,
                end: range.end,
            },
            annotation: Annotation::Literal,
        }));
        spans.sort_by_key(|s| s.span.start);
        linguistics::validate(
            &identity,
            &self.source.text,
            &Candidate {
                source: identity.clone(),
                spans,
            },
        )
        .map_err(|_| fault("analysis"))
    }

    pub fn view(
        &self,
        operation: &str,
        attempt: &str,
    ) -> graph::Result<crate::model::WordGlossView> {
        let analysis = self.validate()?;
        super::super::gloss::project(
            &super::super::gloss::Source {
                identity: analysis.source().clone(),
                text: self.source.text.clone(),
            },
            analysis,
            operation,
            attempt,
        )
        .map_err(|_| fault("projection"))
    }
}

pub(super) fn shape() -> Shape {
    let range = BTreeMap::from([
        ("start".into(), Shape::Integer),
        ("end".into(), Shape::Integer),
    ]);
    let mut text_span = range.clone();
    text_span.insert("text".into(), Shape::Text);
    let texts = Shape::List(Box::new(Shape::Record(text_span)));
    Shape::Record(BTreeMap::from([
        ("source".into(), source_shape()),
        ("targetLanguage".into(), Shape::Text),
        ("explanationLanguage".into(), Shape::Text),
        ("words".into(), texts.clone()),
        (
            "literals".into(),
            Shape::List(Box::new(Shape::Record(range))),
        ),
        ("romanizations".into(), texts.clone()),
        ("pronunciations".into(), texts),
    ]))
}
