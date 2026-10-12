//! Gloss branches of the executable conversation composition. This only builds
//! typed edges; activation and execution remain native runtime responsibilities.
use super::*;
use crate::language::{gloss_graph, linguistics::adapter::settings::Settings};

pub(super) fn extend(definition: &mut Definition, kind: context::Kind) {
    definition.inputs.insert(
        "gloss_settings".into(),
        required(gloss_graph::settings_contract()),
    );
    for (name, port) in graph_text::inputs() {
        definition.inputs.insert(format!("gloss_{name}"), port);
    }
    let mut branches = vec![("reply_gloss", output("reply_source", "source"), vec![])];
    if matches!(kind, context::Kind::Reply) {
        branches.push((
            "learner_gloss",
            Source::Input("learner_source".into()),
            vec!["context".into()],
        ));
    }
    for (name, source, after) in branches {
        let mut inputs: BTreeMap<_, _> = graph_text::inputs()
            .keys()
            .map(|name| (name.clone(), Source::Input(format!("gloss_{name}"))))
            .collect();
        inputs.insert("source".into(), source);
        inputs.insert("settings".into(), Source::Input("gloss_settings".into()));
        definition.nodes.insert(
            name.into(),
            Node {
                operation: gloss_graph::operation_contract(),
                inputs,
                after,
                guard: None,
                activation: Activation::OnDemand,
            },
        );
        definition.outputs.insert(
            name.into(),
            Port {
                contract: gloss_graph::result_contract(),
                optional: true,
            },
        );
        definition
            .results
            .insert(name.into(), output(name, "gloss"));
    }
}

pub(super) fn capture(
    values: &mut Values,
    settings: Settings,
    target: &ResolvedTarget,
    install: &str,
) -> Result<()> {
    for (name, value) in graph_text::capture(
        target,
        install,
        crate::ai::connections::model_routing::TASK_TEMPERATURE,
    )? {
        values.insert(format!("gloss_{name}"), value);
    }
    values.insert("gloss_settings".into(), serde_json::json!(settings));
    Ok(())
}
