use super::*;
use crate::ai::transport::graph_text;
fn output(node: &str, port: &str) -> Source {
    Source::Output {
        node: node.into(),
        port: port.into(),
    }
}
pub(super) fn extend(definition: &mut Definition) {
    definition.nodes.insert(
        "attribution_selection".into(),
        Node {
            operation: attribution_graph::selection_operation(),
            inputs: BTreeMap::from([("assessment".into(), output("assessment", "assessment"))]),
            after: vec![],
            guard: None,
            activation: Activation::Automatic,
        },
    );
    let mut inputs = BTreeMap::from([
        (
            "selection".into(),
            output("attribution_selection", "selection"),
        ),
        ("content".into(), Source::Input("assessment_content".into())),
    ]);
    for (name, port) in graph_text::inputs() {
        let id = format!("attribution_{name}");
        inputs.insert(name, Source::Input(id.clone()));
        definition.inputs.insert(id, port);
    }
    definition.nodes.insert(
        "attribution".into(),
        Node {
            operation: attribution_graph::operation_contract(),
            inputs,
            after: vec![],
            guard: Some(output("attribution_selection", "enabled")),
            activation: Activation::Automatic,
        },
    );
    definition.outputs.insert(
        "attribution".into(),
        Port {
            contract: attribution_graph::result_contract(),
            optional: true,
        },
    );
    definition
        .results
        .insert("attribution".into(), output("attribution", "attribution"));
}
