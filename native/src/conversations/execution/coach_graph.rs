//! Executable coach topology. This artifact is the definition used for scheduling
//! and inspection; there is no separate viewer catalog or SQL dependency list.
use super::{context, prose};
use crate::ai::{connections::access::ResolvedTarget, graph::*};
use std::collections::BTreeMap;

pub const CONTEXT: &str = "context";
pub const REPLY: &str = "reply";
pub const TEXT: &str = "text";

pub fn compile(provider: prose::Provider) -> Result<Executable> {
    let mut registry = Registry::default();
    context::register(&mut registry)?;
    prose::register(&mut registry, provider)?;
    let mut inputs = prose::inputs();
    inputs.get_mut("context").unwrap().contract = context::captured_contract();
    let mut reply_inputs: BTreeMap<_, _> = inputs
        .keys()
        .map(|name| (name.clone(), Source::Input(name.clone())))
        .collect();
    reply_inputs.insert(
        "context".into(),
        Source::Output {
            node: CONTEXT.into(),
            port: "context".into(),
        },
    );
    registry.compile(Definition {
        contract: Contract::new("conversation.coach", 1),
        inputs,
        outputs: prose::outputs(),
        nodes: BTreeMap::from([
            (
                CONTEXT.into(),
                Node {
                    operation: context::operation_contract(),
                    inputs: BTreeMap::from([("context".into(), Source::Input("context".into()))]),
                    after: vec![],
                    guard: None,
                    activation: Activation::Automatic,
                },
            ),
            (
                REPLY.into(),
                Node {
                    operation: prose::operation_contract(),
                    inputs: reply_inputs,
                    after: vec![],
                    guard: None,
                    activation: Activation::Automatic,
                },
            ),
        ]),
        results: BTreeMap::from([(
            TEXT.into(),
            Source::Output {
                node: REPLY.into(),
                port: TEXT.into(),
            },
        )]),
        compositions: BTreeMap::new(),
    })
}

/// Coach admission supplies a reply context, never an opening context. Input
/// capture performs no network work and grants no dispatch/publication authority.
pub fn capture(
    messages: Vec<crate::ai::transport::provider::PromptMessage>,
    source_ids: Vec<String>,
    target: &ResolvedTarget,
    install_id: &str,
) -> Result<Values> {
    prose::capture(
        context::Captured {
            kind: context::Kind::Reply,
            messages,
            source_ids,
        },
        target,
        install_id,
        crate::ai::connections::model_routing::TASK_TEMPERATURE,
    )
}

#[cfg(test)]
mod tests;
