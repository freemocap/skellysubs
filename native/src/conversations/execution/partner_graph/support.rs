use super::*;
use crate::{ai::transport::graph_text, language::source_graph, learning::coaching::support_graph};

pub struct Captured {
    pub context: support_graph::Context,
    pub skills: Option<Vec<support_graph::explanation::Skill>>,
    pub brief_target: ResolvedTarget,
    pub assistance_target: ResolvedTarget,
}
pub(super) fn extend(definition: &mut Definition, kind: context::Kind) {
    definition.inputs.insert(
        "support_context".into(),
        Port {
            contract: support_graph::context_contract(),
            optional: false,
        },
    );
    for (name, task) in [
        ("brief", support_graph::Task::Brief),
        ("assistance", support_graph::Task::Assistance),
    ] {
        let mut inputs = BTreeMap::new();
        for (field, port) in graph_text::inputs() {
            let input = format!("{name}_{field}");
            definition.inputs.insert(input.clone(), port);
            inputs.insert(field, Source::Input(input));
        }
        inputs.insert("context".into(), Source::Input("support_context".into()));
        inputs.insert(
            "source".into(),
            Source::Output {
                node: "reply_source".into(),
                port: "source".into(),
            },
        );
        inputs.insert(
            "learner".into(),
            if matches!(kind, context::Kind::Reply) {
                Source::Input("learner_source".into())
            } else {
                Source::Absent(source_graph::contract())
            },
        );
        definition.nodes.insert(
            name.into(),
            Node {
                operation: task.operation(),
                inputs,
                after: vec![],
                guard: None,
                activation: if task == support_graph::Task::Brief {
                    Activation::Automatic
                } else {
                    Activation::OnDemand
                },
            },
        );
        definition.outputs.insert(
            name.into(),
            Port {
                contract: task.result(),
                optional: true,
            },
        );
        definition.results.insert(
            name.into(),
            Source::Output {
                node: name.into(),
                port: "support".into(),
            },
        );
    }
}
pub(super) fn capture(values: &mut Values, captured: Captured, install: &str) -> Result<()> {
    for (node, target) in [
        ("brief", &captured.brief_target),
        ("assistance", &captured.assistance_target),
    ] {
        for (name, value) in graph_text::capture(
            target,
            install,
            crate::ai::connections::model_routing::TASK_TEMPERATURE,
        )? {
            values.insert(format!("{node}_{name}"), value);
        }
    }
    if let Some(skills) = captured.skills {
        values.insert("explanation_skills".into(), serde_json::json!(skills));
    }
    values.insert(
        "support_context".into(),
        serde_json::json!(captured.context),
    );
    Ok(())
}
