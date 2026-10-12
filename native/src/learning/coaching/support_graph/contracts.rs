use super::*;
pub(super) fn record(fields: impl IntoIterator<Item = (&'static str, Shape)>) -> Shape {
    Shape::Record(
        fields
            .into_iter()
            .map(|(key, shape)| (key.into(), shape))
            .collect(),
    )
}
pub(super) fn list(item: Shape) -> Shape {
    Shape::List(Box::new(item))
}
pub(super) fn register(registry: &mut Registry) -> graph::Result<()> {
    registry.define_type(
        context_contract(),
        record([
            (
                "messages",
                list(record([("role", Shape::Text), ("content", Shape::Text)])),
            ),
            ("targetLanguage", Shape::Text),
            ("translationLanguage", Shape::Text),
            (
                "languageContext",
                record([
                    ("script", Shape::Text),
                    ("guidance", Shape::Map(Box::new(list(Shape::Text)))),
                ]),
            ),
            ("practiceSettings", record([("difficulty", Shape::Text)])),
            (
                "input",
                Shape::Nullable(Box::new(record([
                    ("modality", Shape::Text),
                    ("suggestion", Shape::Boolean),
                    ("scaffold", Shape::Boolean),
                    ("revision", Shape::Boolean),
                ]))),
            ),
        ]),
    )?;
    for task in [Task::Brief, Task::Assistance] {
        let value = match task {
            Task::Brief => record([("explanation", Shape::Text)]),
            Task::Assistance => record([
                (
                    "replies",
                    list(record([
                        ("text", Shape::Text),
                        ("translation", Shape::Text),
                        ("romanization", Shape::Text),
                        ("pronunciation", Shape::Text),
                    ])),
                ),
                ("frames", list(Shape::Text)),
                ("starters", list(Shape::Text)),
            ]),
        };
        registry.define_type(
            task.result(),
            record([("source", source_graph::shape()), ("value", value)]),
        )?;
    }
    Ok(())
}
