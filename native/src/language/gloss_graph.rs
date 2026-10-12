//! One source-bound gloss operation for all reading consumers. Captured language
//! semantics, recovery and projection are shared with existing domain validation.
pub mod result;
use super::{
    linguistics::{self, SourceIdentity, adapter},
    source_graph::{self, SourceText},
};
use crate::ai::{
    connections::access::ResolvedTarget,
    graph::{self, *},
    transport::{graph_evidence, graph_text, provider, text_request::TextRequest},
};
use adapter::settings::Settings;
use serde::{Deserialize, Serialize};
use std::{collections::BTreeMap, future::Future, pin::Pin, sync::Arc};

#[derive(Clone)]
pub struct Request {
    pub source: SourceText,
    pub settings: Settings,
    pub target: ResolvedTarget,
    pub install_id: String,
    pub temperature: f64,
}
fn identity(source: &SourceText, target: &str, explanation: &str) -> SourceIdentity {
    SourceIdentity {
        message_id: source.id.clone(),
        target_language_id: target.into(),
        explanation_language_id: explanation.into(),
        analysis_version: linguistics::ANALYSIS_VERSION.into(),
    }
}
fn fault(path: &str) -> Fault {
    Fault {
        code: "gloss_graph_invalid".into(),
        path: path.into(),
    }
}
impl Request {
    pub fn identity(&self) -> SourceIdentity {
        identity(
            &self.source,
            &self.settings.target_language,
            &self.settings.explanation_language,
        )
    }
    pub fn prompt(&self) -> std::result::Result<adapter::GlossPrompt, adapter::AdapterError> {
        self.settings.prompt(&self.identity(), &self.source.text)
    }
    pub fn text_request(&self, attempt: String, operation: String) -> graph::Result<TextRequest> {
        Ok(TextRequest {
            decisions: None,
            temperature: self.temperature,
            target: self.target.clone(),
            credential: self.target.credential.clone().unwrap_or_default(),
            route: self.target.route,
            model: self.target.model.clone(),
            install_id: self.install_id.clone(),
            attempt,
            operation,
            messages: self.prompt().map_err(|_| fault("prompt"))?.messages,
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

pub fn operation_contract() -> Contract {
    Contract::new("reading.gloss", 1)
}
pub fn settings_contract() -> Contract {
    Contract::new("reading.gloss-settings", 1)
}
pub fn result_contract() -> Contract {
    Contract::new("reading.word-annotations", 1)
}
fn required(contract: Contract) -> Port {
    Port {
        contract,
        optional: false,
    }
}
fn source_shape() -> Shape {
    source_graph::shape()
}
pub fn inputs() -> Ports {
    let mut ports = graph_text::inputs();
    ports.insert("source".into(), required(source_graph::contract()));
    ports.insert("settings".into(), required(settings_contract()));
    ports
}
pub fn outputs() -> Ports {
    BTreeMap::from([("gloss".into(), required(result_contract()))])
}
pub fn capture(
    source: SourceText,
    settings: Settings,
    target: &ResolvedTarget,
    install: &str,
) -> graph::Result<Values> {
    let mut values = graph_text::capture(
        target,
        install,
        crate::ai::connections::model_routing::TASK_TEMPERATURE,
    )?;
    values.insert("source".into(), serde_json::json!(source));
    values.insert("settings".into(), serde_json::json!(settings));
    decode(&values)?;
    Ok(values)
}
pub fn decode(values: &Values) -> graph::Result<Request> {
    let transport = graph_text::decode(values)?;
    let request = Request {
        source: serde_json::from_value(
            values.get("source").ok_or_else(|| fault("source"))?.clone(),
        )
        .map_err(|_| fault("source"))?,
        settings: serde_json::from_value(
            values
                .get("settings")
                .ok_or_else(|| fault("settings"))?
                .clone(),
        )
        .map_err(|_| fault("settings"))?,
        target: transport.target,
        install_id: transport.install_id,
        temperature: transport.temperature,
    };
    request.prompt().map_err(|_| fault("prompt"))?;
    Ok(request)
}

/// The enclosing graph registers shared source/transport types once.
pub fn register(registry: &mut Registry, provider: Provider) -> graph::Result<()> {
    let strings = Shape::List(Box::new(Shape::Text));
    registry.define_type(
        settings_contract(),
        Shape::Record(BTreeMap::from([
            ("targetLanguage".into(), Shape::Text),
            ("explanationLanguage".into(), Shape::Text),
            ("explanationWriting".into(), strings.clone()),
            ("romanization".into(), strings.clone()),
            ("segmentation".into(), strings),
            ("romanizationEnabled".into(), Shape::Boolean),
        ])),
    )?;
    registry.define_type(result_contract(), result::shape())?;
    registry.register(
        Operation {
            contract: operation_contract(),
            implementation: "language/gloss/graph/1".into(),
            inputs: inputs(),
            outputs: outputs(),
            resource: Resource::Provider,
            reuse: Reuse::Exact,
        },
        Arc::new(move |invocation, values| {
            let provider = provider.clone();
            Box::pin(async move {
                let request = decode(&values)?;
                let completion = provider(invocation.clone(), request.clone()).await?;
                let recovered = adapter::recovery::recover_with_settings(
                    &request.identity(),
                    &request.source.text,
                    &completion,
                    &request.settings,
                )
                .map_err(|error| {
                    let domain = crate::model::AppError::new(
                        crate::model::ErrorCode::Provider,
                        "Word meanings failed validation.",
                    )
                    .with_diagnostics(error.diagnostics());
                    invocation
                        .observe(graph_evidence::failure(
                            &domain,
                            &request.target.model,
                            &[&request.source.text, &completion.text],
                        ))
                        .err()
                        .unwrap_or_else(|| fault("response"))
                })?;
                invocation.observe(ResponseEvidence {
                    additional: BTreeMap::from([(
                        "gloss_recovery".into(),
                        EvidenceValue::ClassifiedJson(serde_json::json!({
                            "policy":"word-gloss-recovery-v1", "rejected_spans":recovered.rejected,
                        })),
                    )]),
                    ..Default::default()
                })?;
                let analysis = result::Analysis::capture(request.source, &recovered.analysis);
                Ok(BTreeMap::from([(
                    "gloss".into(),
                    serde_json::json!(analysis),
                )]))
            })
        }),
    )
}

pub fn compile(provider: Provider) -> graph::Result<Executable> {
    let mut registry = Registry::default();
    graph_text::register_types(&mut registry)?;
    source_graph::register(&mut registry)?;
    register(&mut registry, provider)?;
    registry.compile(Definition {
        contract: Contract::new("reading.word-gloss", 1),
        inputs: inputs(),
        outputs: outputs(),
        nodes: BTreeMap::from([(
            "gloss".into(),
            Node {
                operation: operation_contract(),
                inputs: inputs()
                    .keys()
                    .map(|name| (name.clone(), Source::Input(name.clone())))
                    .collect(),
                after: vec![],
                guard: None,
                activation: Activation::OnDemand,
            },
        )]),
        results: BTreeMap::from([(
            "gloss".into(),
            Source::Output {
                node: "gloss".into(),
                port: "gloss".into(),
            },
        )]),
        compositions: BTreeMap::new(),
    })
}

#[cfg(test)]
mod tests;
