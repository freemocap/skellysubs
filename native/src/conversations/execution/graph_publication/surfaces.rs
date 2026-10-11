//! Read-only associations between product fields and executable graph results.
//! No content, attempt provenance, graph construction or persistence is added.
use super::*;
use crate::ai::graph::{InspectionSnapshot, Source};
use std::collections::BTreeMap;

pub(crate) fn surfaces(
    db: &Connection,
    turn: &str,
    graph: &InspectionSnapshot,
) -> Result<BTreeMap<String, Vec<String>>> {
    let effects = db
        .prepare("SELECT node_key,output_port FROM conversation_graph_effects WHERE turn_id=?1")?
        .query_map([turn], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    let definition = &graph.artifact.definition;
    let mut result: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for (name, source) in &definition.results {
        let Source::Output { node, port } = source else {
            continue;
        };
        let operation = &definition.nodes[node].operation;
        let field = if effects.iter().any(|(n, p)| n == node && p == port) {
            Some("text")
        } else {
            helpers::surface(operation, port)
                .or_else(|| learning::surface(operation, port))
                .or_else(|| audio::surface(operation, port))
        };
        if let Some(field) = field {
            // Source-bound helpers declare which message they consume. Follow
            // their explicit source binding; never infer meaning from node names.
            let field = if matches!(field, "translation" | "gloss") {
                let input = definition.nodes[node].inputs.get("source");
                let role = match input {
                    Some(Source::Input(name)) if name == "learner_source" => "learner",
                    Some(Source::Output { node, .. })
                        if definition.nodes[node].operation
                            == super::super::reply_reading_graph::source_binding_contract() =>
                    {
                        "reply"
                    }
                    _ => continue,
                };
                format!("{role}.{field}")
            } else {
                field.to_owned()
            };
            result.entry(field).or_default().push(name.clone());
        }
    }
    Ok(result)
}
