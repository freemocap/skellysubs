use super::*;
use crate::learning::coaching::feedback_graph;

pub struct Captured {
    pub context: feedback_graph::Context,
    pub target: ResolvedTarget,
}
pub(super) fn extend(definition: &mut Definition) {
    let mut bindings = BTreeMap::new();
    for (name, port) in feedback_graph::inputs() {
        let input = if name == "source" {
            "learner_source".into()
        } else {
            let input = format!("feedback_{name}");
            definition.inputs.insert(input.clone(), port);
            input
        };
        bindings.insert(name, Source::Input(input));
    }
    definition.nodes.insert(
        "feedback".into(),
        Node {
            operation: feedback_graph::operation_contract(),
            inputs: bindings,
            after: vec!["context".into()],
            guard: None,
            activation: Activation::Automatic,
        },
    );
    definition.outputs.insert(
        "feedback".into(),
        Port {
            contract: feedback_graph::result_contract(),
            optional: true,
        },
    );
    definition.results.insert(
        "feedback".into(),
        Source::Output {
            node: "feedback".into(),
            port: "feedback".into(),
        },
    );
}
pub(super) fn capture(
    reply: &reply_reading_graph::Captured,
    captured: Option<Captured>,
) -> Result<Values> {
    let invalid = || Fault {
        code: "partner_graph_capture_invalid".into(),
        path: "feedback".into(),
    };
    match (reply.context.kind, captured) {
        (context::Kind::Reply, Some(captured)) => {
            if serde_json::to_value(&captured.context.messages).map_err(|_| invalid())?
                != serde_json::to_value(&reply.context.messages).map_err(|_| invalid())?
            {
                return Err(invalid());
            }
            Ok(feedback_graph::capture(
                reply.learner_source.clone().ok_or_else(invalid)?,
                captured.context,
                &captured.target,
                &reply.install_id,
            )?
            .into_iter()
            .filter(|(name, _)| name != "source")
            .map(|(name, value)| (format!("feedback_{name}"), value))
            .collect())
        }
        (context::Kind::Opening | context::Kind::SeededOpening, None) => Ok(BTreeMap::new()),
        _ => Err(invalid()),
    }
}
