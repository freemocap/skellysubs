//! Reusable source-bound translation operation. Node names and speaker roles do
//! not select implementations. The host owns credentials, admission, wire IDs
//! and current source authority; this operation owns semantic response validation.
use super::{
    source_graph::{self, SourceText},
    translation,
};
use crate::ai::{
    connections::access::ResolvedTarget,
    graph::{self, *},
    transport::{graph_evidence, graph_text, provider, text_request::TextRequest},
};
use serde::{Deserialize, Serialize};
use std::{collections::BTreeMap, future::Future, pin::Pin, sync::Arc};

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct Languages {
    pub source: String,
    pub destination: String,
    pub destination_writing: Vec<String>,
}

/// All computation inputs are captured ports. Source ownership is checked by
/// the host, not inferred from the text or discovered through a database read.
#[derive(Clone)]
pub struct Request {
    pub source: SourceText,
    pub languages: Languages,
    pub target: ResolvedTarget,
    pub install_id: String,
    pub temperature: f64,
}

impl Request {
    pub fn text_request(&self, attempt: String, operation: String) -> TextRequest {
        let semantic = translation::Request {
            source: self.source.text.clone(),
            source_language: self.languages.source.clone(),
            destination_language: self.languages.destination.clone(),
            destination_writing: self.languages.destination_writing.clone(),
        };
        TextRequest {
            decisions: None,
            temperature: self.temperature,
            target: self.target.clone(),
            credential: self.target.credential.clone().unwrap_or_default(),
            route: self.target.route,
            model: self.target.model.clone(),
            install_id: self.install_id.clone(),
            attempt,
            operation,
            messages: semantic.messages(),
        }
    }
    pub fn schema(&self) -> serde_json::Value {
        translation::schema()
    }
}

/// Return the original completion. The host must retain transport evidence and
/// failures through the supplied invocation before returning a core Fault.
pub type Provider = Arc<
    dyn Fn(
            InvocationContext,
            Request,
        ) -> Pin<Box<dyn Future<Output = graph::Result<provider::Completion>> + Send>>
        + Send
        + Sync,
>;

fn contract(name: &str) -> Contract {
    Contract::new(name, 1)
}
pub fn operation_contract() -> Contract {
    contract("reading.translate")
}
pub fn source_contract() -> Contract {
    source_graph::contract()
}
pub fn languages_contract() -> Contract {
    contract("reading.translation-languages")
}
pub fn result_contract() -> Contract {
    contract("reading.bound-translation")
}
fn required(contract: Contract) -> Port {
    Port {
        contract,
        optional: false,
    }
}
fn fault(path: &str) -> Fault {
    Fault {
        code: "translation_input_invalid".into(),
        path: path.into(),
    }
}

pub fn inputs() -> Ports {
    let mut ports = graph_text::inputs();
    ports.insert("source".into(), required(source_contract()));
    ports.insert("languages".into(), required(languages_contract()));
    ports
}
pub fn outputs() -> Ports {
    BTreeMap::from([("translation".into(), required(result_contract()))])
}

pub fn capture(
    source: SourceText,
    languages: Languages,
    target: &ResolvedTarget,
    install_id: &str,
) -> graph::Result<Values> {
    let mut values = graph_text::capture(
        target,
        install_id,
        crate::ai::connections::model_routing::TASK_TEMPERATURE,
    )?;
    values.insert(
        "source".into(),
        serde_json::to_value(source).map_err(|_| fault("source"))?,
    );
    values.insert(
        "languages".into(),
        serde_json::to_value(languages).map_err(|_| fault("languages"))?,
    );
    decode(&values)?;
    Ok(values)
}

pub fn decode(values: &Values) -> graph::Result<Request> {
    let settings = graph_text::decode(values)?;
    let source: SourceText =
        serde_json::from_value(values.get("source").ok_or_else(|| fault("source"))?.clone())
            .map_err(|_| fault("source"))?;
    let languages: Languages = serde_json::from_value(
        values
            .get("languages")
            .ok_or_else(|| fault("languages"))?
            .clone(),
    )
    .map_err(|_| fault("languages"))?;
    if source.id.is_empty() || source.text.trim().is_empty() {
        return Err(fault("source"));
    }
    if languages.source.trim().is_empty() || languages.destination.trim().is_empty() {
        return Err(fault("languages"));
    }
    Ok(Request {
        source,
        languages,
        target: settings.target,
        install_id: settings.install_id,
        temperature: settings.temperature,
    })
}

/// Register shared transport types once at graph composition, then register this
/// operation once irrespective of the number of translation nodes in that graph.
pub fn register(registry: &mut Registry, provider: Provider) -> graph::Result<()> {
    registry.define_type(
        languages_contract(),
        Shape::Record(BTreeMap::from([
            ("source".into(), Shape::Text),
            ("destination".into(), Shape::Text),
            (
                "destinationWriting".into(),
                Shape::List(Box::new(Shape::Text)),
            ),
        ])),
    )?;
    registry.define_type(
        result_contract(),
        Shape::Record(BTreeMap::from([
            (
                "source".into(),
                Shape::Record(BTreeMap::from([
                    ("id".into(), Shape::Text),
                    ("text".into(), Shape::Text),
                ])),
            ),
            ("text".into(), Shape::Text),
        ])),
    )?;
    registry.register(
        Operation {
            contract: operation_contract(),
            implementation: "language/translation/graph/1".into(),
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
                let text =
                    translation::validate(&request.source.text, &completion).map_err(|error| {
                        invocation
                            .observe(graph_evidence::failure(
                                &error,
                                &request.target.model,
                                &[&request.source.text, &completion.text],
                            ))
                            .err()
                            .unwrap_or(Fault {
                                code: "translation_response_invalid".into(),
                                path: "translation".into(),
                            })
                    })?;
                Ok(BTreeMap::from([(
                    "translation".into(),
                    serde_json::json!({
                        "source":request.source,"text":text,
                    }),
                )]))
            })
        }),
    )
}

/// A reusable boundary for explicit reading requests. Conversation compositions
/// register the same operation and connect their source-producing nodes to it.
pub fn compile(provider: Provider) -> graph::Result<Executable> {
    let mut registry = Registry::default();
    graph_text::register_types(&mut registry)?;
    source_graph::register(&mut registry)?;
    register(&mut registry, provider)?;
    registry.compile(Definition {
        contract: contract("reading.translation"),
        inputs: inputs(),
        outputs: outputs(),
        nodes: BTreeMap::from([(
            "translate".into(),
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
            "translation".into(),
            Source::Output {
                node: "translate".into(),
                port: "translation".into(),
            },
        )]),
        compositions: BTreeMap::new(),
    })
}

#[cfg(test)]
mod tests;
