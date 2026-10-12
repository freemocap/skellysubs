//! Partner workflow composition in progress. The native artifact owns all
//! dependencies; application admission switches only when its remaining branches
//! and publication adapters are connected.
mod attribution;
pub mod captured_turn;
mod explanations;
pub mod feedback;
mod speech;
pub mod support;
use super::{context, prose, reply_reading_graph};
use crate::{
    ai::{connections::access::ResolvedTarget, graph::*},
    language::{gloss_graph, translation_graph},
    learning::coaching::{assessment_graph, attribution_graph, feedback_graph, support_graph},
};
use std::collections::BTreeMap;

pub struct Providers {
    pub reply: prose::Provider,
    pub translation: translation_graph::Provider,
    pub gloss: gloss_graph::Provider,
    pub assessment: assessment_graph::Provider,
    pub attribution: attribution_graph::Provider,
    pub support: support_graph::Provider,
    pub feedback: feedback_graph::Provider,
    pub explanations: support_graph::explanation::Provider,
    pub available_evidence: support_graph::explanation_snapshot::Reader,
    pub speech: crate::speech::synthesis_graph::Provider,
    pub audio_lookup: crate::speech::synthesis_graph::playback::Lookup,
}
pub fn compile(kind: context::Kind, providers: Providers) -> Result<Executable> {
    let mut registry = Registry::default();
    reply_reading_graph::register(
        &mut registry,
        providers.reply,
        providers.translation,
        providers.gloss,
    )?;
    assessment_graph::register(&mut registry, providers.assessment)?;
    attribution_graph::register(&mut registry, providers.attribution)?;
    support_graph::register(&mut registry, providers.support)?;
    feedback_graph::register(&mut registry, providers.feedback)?;
    support_graph::explanation::register(&mut registry, providers.explanations)?;
    support_graph::explanation_snapshot::register(&mut registry, providers.available_evidence)?;
    let mut definition = reply_reading_graph::definition(kind);
    definition.contract = Contract::new(
        if matches!(kind, context::Kind::Reply) {
            "conversation.partner-reply"
        } else {
            "conversation.partner-opening"
        },
        1,
    );
    if matches!(kind, context::Kind::Reply) {
        let mut bindings = BTreeMap::new();
        for (name, port) in assessment_graph::inputs() {
            if name == "source" {
                bindings.insert(name, Source::Input("learner_source".into()));
            } else {
                let input = format!("assessment_{name}");
                bindings.insert(name, Source::Input(input.clone()));
                definition.inputs.insert(input, port);
            }
        }
        definition.nodes.insert(
            "assessment".into(),
            Node {
                operation: assessment_graph::operation_contract(),
                inputs: bindings,
                after: vec!["context".into()],
                guard: None,
                activation: Activation::Automatic,
            },
        );
        definition.outputs.insert(
            "assessment".into(),
            Port {
                contract: assessment_graph::result_contract(),
                optional: true,
            },
        );
        definition.results.insert(
            "assessment".into(),
            Source::Output {
                node: "assessment".into(),
                port: "assessment".into(),
            },
        );
    }
    if matches!(kind, context::Kind::Reply) {
        attribution::extend(&mut definition);
        feedback::extend(&mut definition);
    }
    support::extend(&mut definition, kind);
    explanations::extend(&mut definition, kind);
    speech::extend(
        &mut registry,
        &mut definition,
        providers.speech,
        providers.audio_lookup,
    )?;
    registry.compile(definition)
}

pub struct AssessmentCapture {
    pub context: assessment_graph::Context,
    pub content: Option<assessment_graph::Content>,
    pub target: ResolvedTarget,
}
pub fn capture(
    reply: reply_reading_graph::Captured,
    assessment: Option<AssessmentCapture>,
    support: support::Captured,
    feedback: Option<feedback::Captured>,
) -> Result<Values> {
    let invalid = || Fault {
        code: "partner_graph_capture_invalid".into(),
        path: "assessment".into(),
    };
    if serde_json::to_value(&support.context.messages).map_err(|_| invalid())?
        != serde_json::to_value(&reply.context.messages).map_err(|_| invalid())?
    {
        return Err(Fault {
            code: "partner_graph_capture_invalid".into(),
            path: "support.context.messages".into(),
        });
    }
    let feedback_values = feedback::capture(&reply, feedback)?;
    let install = reply.install_id.clone();
    let mut attribution_values = BTreeMap::new();
    let values = match (&reply.context.kind, assessment) {
        (context::Kind::Reply, Some(assessment)) => {
            // Both branches must see the same captured exchange, not independently
            // reconstructed or differently truncated histories.
            if serde_json::to_value(&assessment.context.messages).map_err(|_| invalid())?
                != serde_json::to_value(&reply.context.messages).map_err(|_| invalid())?
            {
                return Err(invalid());
            }
            attribution_values = crate::ai::transport::graph_text::capture(
                &assessment.target,
                &reply.install_id,
                crate::ai::connections::model_routing::TASK_TEMPERATURE,
            )?;
            assessment_graph::capture_available(
                reply.learner_source.clone().ok_or_else(invalid)?,
                assessment.context,
                assessment.content,
                &assessment.target,
                &reply.install_id,
            )?
        }
        (context::Kind::Opening | context::Kind::SeededOpening, None) => BTreeMap::new(),
        _ => return Err(invalid()),
    };
    let mut captured = reply_reading_graph::capture(reply)?;
    for (name, value) in values {
        if name != "source" {
            captured.insert(format!("assessment_{name}"), value);
        }
    }
    for (name, value) in attribution_values {
        captured.insert(format!("attribution_{name}"), value);
    }
    captured.extend(feedback_values);
    support::capture(&mut captured, support, &install)?;
    Ok(captured)
}

#[cfg(test)]
mod tests;
