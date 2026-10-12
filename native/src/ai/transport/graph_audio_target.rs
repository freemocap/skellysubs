//! The captured audio route shape, shared without changing existing artifacts.
use crate::ai::{connections::access::ResolvedTarget, graph::Shape};
use std::collections::BTreeMap;
pub fn shape() -> Shape {
    Shape::Record(BTreeMap::from([
        (
            "audio_resolution".into(),
            Shape::Nullable(Box::new(Shape::Record(
                [
                    "requested_model",
                    "model",
                    "provider",
                    "language_code",
                    "reason",
                ]
                .into_iter()
                .map(|k| (k.into(), Shape::Text))
                .collect(),
            ))),
        ),
        ("route".into(), Shape::Text),
        ("revision".into(), Shape::Integer),
        ("url".into(), Shape::Text),
        ("model".into(), Shape::Text),
        ("credential".into(), Shape::Nullable(Box::new(Shape::Text))),
    ]))
}
pub fn capture(target: &ResolvedTarget) -> crate::model::Result<serde_json::Value> {
    let mut value = serde_json::to_value(target)?;
    if value.get("audio_resolution").is_none() {
        value["audio_resolution"] = serde_json::Value::Null;
    }
    Ok(value)
}
