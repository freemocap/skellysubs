//! Exact text and domain identity shared by source-bound reading operations.
use crate::ai::graph::{Contract, Registry, Result, Shape};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct SourceText {
    pub id: String,
    pub text: String,
}
pub fn contract() -> Contract {
    Contract::new("reading.source-text", 1)
}
pub fn shape() -> Shape {
    Shape::Record(BTreeMap::from([
        ("id".into(), Shape::Text),
        ("text".into(), Shape::Text),
    ]))
}
pub fn register(registry: &mut Registry) -> Result<()> {
    registry.define_type(contract(), shape())
}
