use super::*;
fn record(fields: impl IntoIterator<Item = (&'static str, Shape)>) -> Shape {
    Shape::Record(
        fields
            .into_iter()
            .map(|(key, shape)| (key.into(), shape))
            .collect(),
    )
}
fn list(shape: Shape) -> Shape {
    Shape::List(Box::new(shape))
}
fn optional(shape: Shape) -> Shape {
    Shape::Nullable(Box::new(shape))
}
pub(super) fn register(registry: &mut Registry) -> graph::Result<()> {
    registry.define_type(
        context_contract(),
        record([
            (
                "messages",
                list(record([("role", Shape::Text), ("content", Shape::Text)])),
            ),
            (
                "input",
                record([
                    ("modality", Shape::Text),
                    ("suggestion", Shape::Boolean),
                    ("scaffold", Shape::Boolean),
                    ("revision", Shape::Boolean),
                ]),
            ),
            ("targetLanguage", Shape::Text),
            ("translationLanguage", Shape::Text),
            (
                "candidateConstructs",
                list(record([("id", Shape::Text), ("criterion", Shape::Text)])),
            ),
            (
                "practiceSettings",
                record([
                    ("difficulty", Shape::Text),
                    ("coachProactivity", Shape::Text),
                ]),
            ),
            ("practiceFocus", optional(record([("id", Shape::Text)]))),
            ("feedbackContext", optional(Shape::Text)),
            (
                "feedbackPolicy",
                record([
                    ("correct_only", Shape::Text),
                    ("skip_sources", list(Shape::Text)),
                    (
                        "intensity",
                        Shape::Map(Box::new(record([
                            ("start_at", Shape::Text),
                            ("show_logged", Shape::Boolean),
                        ]))),
                    ),
                    ("never", list(Shape::Text)),
                    ("sources", list(Shape::Text)),
                ]),
            ),
            ("guidance", Shape::Map(Box::new(list(Shape::Text)))),
        ]),
    )?;
    let error = record([
        ("op", Shape::Text),
        ("category", Shape::Text),
        ("source", Shape::Text),
        ("blocks_meaning", Shape::Boolean),
        ("target_hypothesis", Shape::Text),
        ("hint", Shape::Text),
        ("elicitation", Shape::Text),
        ("metalinguistic", Shape::Text),
    ]);
    let item = record([
        ("construct", Shape::Text),
        ("quote", Shape::Text),
        ("outcome", Shape::Text),
        ("error", optional(error)),
        ("rationale", Shape::Text),
    ]);
    let summary = record([
        ("construct", Shape::Text),
        ("quote", Shape::Text),
        ("outcome", Shape::Text),
        ("rationale", Shape::Text),
    ]);
    let correction = record([
        ("construct", Shape::Text),
        ("quote", Shape::Text),
        ("move", Shape::Text),
        ("text", Shape::Text),
        ("explanation", optional(Shape::Text)),
    ]);
    registry.define_type(
        result_contract(),
        record([
            ("source", source_graph::shape()),
            (
                "value",
                record([
                    (
                        "observation",
                        record([("meaning_recovered", Shape::Text), ("items", list(item))]),
                    ),
                    (
                        "decision",
                        record([
                            ("exposedMove", optional(Shape::Text)),
                            ("shown", optional(correction)),
                            ("retryInvited", Shape::Boolean),
                            ("alsoNoticed", list(summary)),
                            ("keptGoing", Shape::Boolean),
                        ]),
                    ),
                    ("validationOmissions", Shape::Integer),
                ]),
            ),
        ]),
    )
}
