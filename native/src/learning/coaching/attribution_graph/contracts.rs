use super::*;
fn record(fields: impl IntoIterator<Item = (&'static str, Shape)>) -> Shape {
    Shape::Record(
        fields
            .into_iter()
            .map(|(key, shape)| (key.into(), shape))
            .collect(),
    )
}
pub(super) fn register(registry: &mut Registry) -> graph::Result<()> {
    registry.define_type(enabled_contract(), Shape::Boolean)?;
    registry.define_type(
        selection_contract(),
        record([
            ("source", source_graph::shape()),
            ("selected", Shape::List(Box::new(Shape::Text))),
        ]),
    )?;
    registry.define_type(
        result_contract(),
        record([
            ("source", source_graph::shape()),
            (
                "attribution",
                record([
                    ("model", Shape::Text),
                    (
                        "skills",
                        Shape::Map(Box::new(record([
                            ("evidence_kind", Shape::Text),
                            ("reason", Shape::Text),
                            (
                                "spans",
                                Shape::List(Box::new(record([
                                    ("quote", Shape::Text),
                                    ("start", Shape::Integer),
                                    ("end", Shape::Integer),
                                ]))),
                            ),
                        ]))),
                    ),
                ]),
            ),
        ]),
    )
}
