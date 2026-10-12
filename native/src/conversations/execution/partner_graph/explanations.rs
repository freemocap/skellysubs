use super::*;
use crate::{
    ai::transport::graph_text,
    language::source_graph,
    learning::coaching::support_graph::{explanation, explanation_snapshot},
};

pub(super) fn extend(definition: &mut Definition, kind: context::Kind) {
    definition.inputs.insert(
        "explanation_skills".into(),
        Port {
            contract: explanation_snapshot::skills_contract(),
            optional: true,
        },
    );
    let learner = if matches!(kind, context::Kind::Reply) {
        Source::Input("learner_source".into())
    } else {
        Source::Absent(source_graph::contract())
    };
    definition.nodes.insert(
        "explanation_evidence".into(),
        Node {
            operation: explanation_snapshot::operation_contract(),
            inputs: BTreeMap::from([
                ("source".into(), learner.clone()),
                ("skills".into(), Source::Input("explanation_skills".into())),
            ]),
            // A help request concerns the actual reply. Wait for its adoption before
            // taking the evidence snapshot, but never wait for learner assessment.
            after: vec!["reply_source".into()],
            guard: None,
            activation: Activation::OnDemand,
        },
    );
    let mut inputs: BTreeMap<_, _> = graph_text::inputs()
        .keys()
        .map(|name| (name.clone(), Source::Input(format!("assistance_{name}"))))
        .collect();
    inputs.extend([
        (
            "source".into(),
            Source::Output {
                node: "reply_source".into(),
                port: "source".into(),
            },
        ),
        ("learner".into(), learner),
        ("context".into(), Source::Input("support_context".into())),
        (
            "learning".into(),
            Source::Output {
                node: "explanation_evidence".into(),
                port: "learning".into(),
            },
        ),
    ]);
    definition.nodes.insert(
        "explanations".into(),
        Node {
            operation: explanation::operation_contract(),
            inputs,
            after: vec![],
            guard: None,
            activation: Activation::Automatic,
        },
    );
    definition.outputs.insert(
        "explanations".into(),
        Port {
            contract: explanation::result_contract(),
            optional: true,
        },
    );
    definition.results.insert(
        "explanations".into(),
        Source::Output {
            node: "explanations".into(),
            port: "support".into(),
        },
    );
}
