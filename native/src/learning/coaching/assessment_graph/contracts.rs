use super::*;
fn record(fields: impl IntoIterator<Item = (&'static str, Shape)>) -> Shape {
    Shape::Record(
        fields
            .into_iter()
            .map(|(key, value)| (key.into(), value))
            .collect(),
    )
}
fn text_fields(names: &[&'static str]) -> Shape {
    record(names.iter().map(|name| (*name, Shape::Text)))
}
fn map(shape: Shape) -> Shape {
    Shape::Map(Box::new(shape))
}
pub(super) fn register(registry: &mut Registry) -> graph::Result<()> {
    registry.define_type(
        context_contract(),
        record([
            (
                "messages",
                Shape::List(Box::new(text_fields(&["role", "content"]))),
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
            ("language", Shape::Text),
            ("variety", Shape::Text),
        ]),
    )?;
    let question = record([
        ("instructions", Shape::Text),
        ("criteria", map(Shape::Text)),
    ]);
    registry.define_type(
        content_contract(),
        record([
            (
                "skills",
                Shape::List(Box::new(text_fields(&[
                    "id",
                    "name",
                    "overview",
                    "boundary",
                    "language_guidance",
                ]))),
            ),
            (
                "instructions",
                record([
                    (
                        "attribution",
                        record([
                            ("minimum_positive_probability", Shape::Number),
                            ("instructions", Shape::Text),
                        ]),
                    ),
                    ("instructions", Shape::Text),
                    ("question", Shape::Text),
                    ("criteria", map(Shape::Text)),
                ]),
            ),
            (
                "questions",
                record([
                    ("grammar", question.clone()),
                    ("understandability", question),
                ]),
            ),
        ]),
    )?;
    let answer = record([
        ("choice", Shape::Text),
        ("probabilities", map(Shape::Number)),
        ("confidence", Shape::Number),
    ]);
    registry.define_type(
        result_contract(),
        record([
            ("source", source_graph::shape()),
            (
                "assessment",
                record([
                    ("presence", map(Shape::Text)),
                    ("answers", map(answer.clone())),
                    ("grammar", answer.clone()),
                    ("understandability", answer),
                    ("model", Shape::Text),
                    ("adapter", Shape::Text),
                    ("promptVersion", Shape::Text),
                    (
                        "policy",
                        record([
                            ("version", Shape::Text),
                            ("minimumPositiveProbability", Shape::Number),
                        ]),
                    ),
                ]),
            ),
        ]),
    )
}
