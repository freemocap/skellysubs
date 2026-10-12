//! Source-bound coaching advice, independent of assessment credit and publication.
mod contracts;
use super::{InputEvidence, coach_observation};
use crate::{
    ai::{
        connections::access::ResolvedTarget,
        graph::{self, *},
        transport::{graph_evidence, graph_text, provider, text_request::TextRequest},
    },
    language::source_graph::{self, SourceText},
};
use serde::{Deserialize, Serialize};
use std::{collections::BTreeMap, future::Future, pin::Pin, sync::Arc};

#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Candidate {
    pub id: String,
    pub criterion: String,
}
#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Practice {
    pub difficulty: String,
    pub coach_proactivity: String,
}
#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Focus {
    pub id: String,
}
#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Context {
    pub messages: Vec<provider::PromptMessage>,
    pub input: InputEvidence,
    pub target_language: String,
    pub translation_language: String,
    pub candidate_constructs: Vec<Candidate>,
    pub practice_settings: Practice,
    pub practice_focus: Option<Focus>,
    pub feedback_context: Option<String>,
    pub feedback_policy: crate::configuration::FeedbackPolicy,
    pub guidance: BTreeMap<String, Vec<String>>,
}
#[derive(Clone)]
pub struct Request {
    pub source: SourceText,
    pub context: Context,
    pub target: ResolvedTarget,
    pub install_id: String,
    pub temperature: f64,
}
impl Request {
    pub fn schema(&self) -> crate::model::Result<serde_json::Value> {
        coach_observation::schema(&serde_json::json!(self.context))
    }
    pub fn text_request(
        &self,
        attempt: String,
        operation: String,
    ) -> crate::model::Result<TextRequest> {
        let captured = serde_json::json!(self.context);
        let system =
            super::system_prompt_with_guidance(super::FEEDBACK, &captured, &self.context.guidance)?;
        let messages = super::prompt_for_sources(
            Some(self.source.text.clone()),
            None,
            super::FEEDBACK,
            &captured,
            system,
        )?;
        Ok(TextRequest {
            decisions: None,
            temperature: self.temperature,
            credential: self.target.credential.clone().unwrap_or_default(),
            model: self.target.model.clone(),
            route: self.target.route,
            target: self.target.clone(),
            install_id: self.install_id.clone(),
            attempt,
            operation,
            messages,
        })
    }
}
pub type Provider = Arc<
    dyn Fn(
            InvocationContext,
            Request,
        ) -> Pin<Box<dyn Future<Output = graph::Result<provider::Completion>> + Send>>
        + Send
        + Sync,
>;
fn fault(path: &str) -> Fault {
    Fault {
        code: "feedback_graph_invalid".into(),
        path: path.into(),
    }
}
pub fn operation_contract() -> Contract {
    Contract::new("learning.observe-message", 1)
}
pub fn context_contract() -> Contract {
    Contract::new("learning.feedback-context", 1)
}
pub fn result_contract() -> Contract {
    Contract::new("learning.bound-feedback", 1)
}
fn required(contract: Contract) -> Port {
    Port {
        contract,
        optional: false,
    }
}
pub fn inputs() -> Ports {
    let mut ports = graph_text::inputs();
    ports.insert("source".into(), required(source_graph::contract()));
    ports.insert("context".into(), required(context_contract()));
    ports
}
pub fn capture(
    source: SourceText,
    context: Context,
    target: &ResolvedTarget,
    install: &str,
) -> graph::Result<Values> {
    let mut values = graph_text::capture(
        target,
        install,
        crate::ai::connections::model_routing::TASK_TEMPERATURE,
    )?;
    values.insert("source".into(), serde_json::json!(source));
    values.insert("context".into(), serde_json::json!(context));
    decode(&values)?;
    Ok(values)
}
pub fn decode(values: &Values) -> graph::Result<Request> {
    let settings = graph_text::decode(values)?;
    let request = Request {
        source: serde_json::from_value(
            values
                .get("source")
                .cloned()
                .ok_or_else(|| fault("source"))?,
        )
        .map_err(|_| fault("source"))?,
        context: serde_json::from_value(
            values
                .get("context")
                .cloned()
                .ok_or_else(|| fault("context"))?,
        )
        .map_err(|_| fault("context"))?,
        target: settings.target,
        install_id: settings.install_id,
        temperature: settings.temperature,
    };
    if request.source.id.is_empty()
        || request.source.text.trim().is_empty()
        || !request
            .context
            .messages
            .last()
            .is_some_and(|m| m.role == "user" && m.content == request.source.text)
    {
        return Err(fault("source"));
    }
    request.schema().map_err(|_| fault("schema"))?;
    request
        .text_request(String::new(), String::new())
        .map_err(|_| fault("prompt"))?;
    Ok(request)
}
pub fn register(registry: &mut Registry, provider: Provider) -> graph::Result<()> {
    contracts::register(registry)?;
    registry.register(
        Operation {
            contract: operation_contract(),
            implementation: "learning/feedback/graph/1".into(),
            inputs: inputs(),
            outputs: BTreeMap::from([("feedback".into(), required(result_contract()))]),
            resource: Resource::Provider,
            reuse: Reuse::Exact,
        },
        Arc::new(move |invocation, values| {
            let provider = provider.clone();
            Box::pin(async move {
                let request = decode(&values)?;
                let completion = provider(invocation.clone(), request.clone()).await?;
                let mut feedback = coach_observation::validate_captured(
                    &serde_json::json!(request.context),
                    super::FEEDBACK,
                    &completion,
                )
                .map_err(|error| {
                    invocation
                        .observe(graph_evidence::failure(
                            &error,
                            &request.target.model,
                            &[&request.source.text, &completion.text],
                        ))
                        .err()
                        .unwrap_or_else(|| fault("response"))
                })?;
                // The domain's optional explanation is omitted in its display JSON;
                // the graph's closed record represents that same absence with null.
                if let Some(shown) = feedback["decision"]["shown"].as_object_mut() {
                    shown
                        .entry("explanation")
                        .or_insert(serde_json::Value::Null);
                }
                Ok(BTreeMap::from([(
                    "feedback".into(),
                    serde_json::json!({"source":request.source,"value":feedback}),
                )]))
            })
        }),
    )
}

/// The same feedback operation can run as an explicit source-bound inquiry.
pub fn compile(provider: Provider) -> graph::Result<Executable> {
    let mut registry = Registry::default();
    graph_text::register_types(&mut registry)?;
    source_graph::register(&mut registry)?;
    register(&mut registry, provider)?;
    registry.compile(Definition {
        contract: Contract::new("learning.feedback-inquiry", 1),
        inputs: inputs(),
        outputs: BTreeMap::from([("feedback".into(), required(result_contract()))]),
        nodes: BTreeMap::from([(
            "feedback".into(),
            Node {
                operation: operation_contract(),
                inputs: inputs()
                    .keys()
                    .map(|name| (name.clone(), Source::Input(name.clone())))
                    .collect(),
                after: vec![],
                guard: None,
                activation: Activation::Automatic,
            },
        )]),
        results: BTreeMap::from([(
            "feedback".into(),
            Source::Output {
                node: "feedback".into(),
                port: "feedback".into(),
            },
        )]),
        compositions: BTreeMap::new(),
    })
}

#[cfg(test)]
mod tests;
