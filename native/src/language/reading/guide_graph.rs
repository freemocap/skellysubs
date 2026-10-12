//! Guide prose translation. Examples remain authored; only declared fields return.
use crate::ai::{
    graph::{self, *},
    transport::{graph_evidence, graph_text, provider, text_request::TextRequest},
};
use serde_json::json;
use std::{collections::BTreeMap, future::Future, pin::Pin, sync::Arc};

#[derive(Clone)]
pub struct Request {
    messages: Vec<provider::PromptMessage>,
    count: usize,
    settings: graph_text::Settings,
}
impl Request {
    pub fn prepare(&self, attempt: String, operation: String) -> (TextRequest, serde_json::Value) {
        (
            TextRequest {
                decisions: None,
                temperature: self.settings.temperature,
                credential: self.settings.target.credential.clone().unwrap_or_default(),
                model: self.settings.target.model.clone(),
                route: self.settings.target.route,
                target: self.settings.target.clone(),
                install_id: self.settings.install_id.clone(),
                attempt,
                operation,
                messages: self.messages.clone(),
            },
            crate::configuration::guide_translation::translation_schema(self.count),
        )
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
pub fn operation() -> Contract {
    Contract::new("reading.translate-guide", 1)
}
fn required(name: &str) -> Port {
    Port {
        contract: Contract::new(name, 1),
        optional: false,
    }
}
fn fault(path: &str) -> Fault {
    Fault {
        code: "guide_translation_invalid".into(),
        path: path.into(),
    }
}
fn inputs() -> Ports {
    let mut ports = graph_text::inputs();
    ports.insert("messages".into(), required("reading.guide-messages"));
    ports.insert("field_count".into(), required("reading.guide-field-count"));
    ports
}
pub fn capture(
    request: &crate::ai::generation::Request,
    messages: Vec<provider::PromptMessage>,
    count: usize,
) -> graph::Result<Values> {
    let mut values = graph_text::capture(
        &request.target,
        &request.install_id,
        crate::ai::connections::model_routing::TASK_TEMPERATURE,
    )?;
    values.insert("messages".into(), json!(messages));
    values.insert("field_count".into(), json!(count));
    decode(&values)?;
    Ok(values)
}
pub fn decode(values: &Values) -> graph::Result<Request> {
    let get = |key: &str| values.get(key).cloned().ok_or_else(|| fault(key));
    let messages: Vec<provider::PromptMessage> =
        serde_json::from_value(get("messages")?).map_err(|_| fault("messages"))?;
    let count: usize =
        serde_json::from_value(get("field_count")?).map_err(|_| fault("field_count"))?;
    if messages.is_empty() || count == 0 {
        return Err(fault("request"));
    }
    Ok(Request {
        messages,
        count,
        settings: graph_text::decode(values)?,
    })
}
pub fn compile(provider: Provider) -> graph::Result<Executable> {
    let mut registry = Registry::default();
    graph_text::register_types(&mut registry)?;
    for (name, shape) in [
        (
            "reading.guide-messages",
            Shape::List(Box::new(Shape::Record(BTreeMap::from([
                ("role".into(), Shape::Text),
                ("content".into(), Shape::Text),
            ])))),
        ),
        ("reading.guide-field-count", Shape::Integer),
        (
            "reading.guide-translated-fields",
            Shape::List(Box::new(Shape::Text)),
        ),
    ] {
        registry.define_type(Contract::new(name, 1), shape)?;
    }
    let outputs = BTreeMap::from([("texts".into(), required("reading.guide-translated-fields"))]);
    registry.register(
        Operation {
            contract: operation(),
            implementation: "reading/guide-translation/1".into(),
            inputs: inputs(),
            outputs: outputs.clone(),
            resource: Resource::Provider,
            reuse: Reuse::Fresh,
        },
        Arc::new(move |invocation, values| {
            let provider = provider.clone();
            Box::pin(async move {
                let request = decode(&values)?;
                let completion = provider(invocation.clone(), request.clone()).await?;
                let result = if completion.finish_reason != "stop" {
                    Err(crate::model::AppError::new(
                        crate::model::ErrorCode::Provider,
                        "Guide translation did not finish normally.",
                    ))
                } else {
                    crate::configuration::guide_translation::decode_texts(
                        &completion.text,
                        request.count,
                    )
                };
                let texts = result.map_err(|error| {
                    invocation
                        .observe(graph_evidence::failure(
                            &error,
                            &request.settings.target.model,
                            &[&completion.text],
                        ))
                        .err()
                        .unwrap_or_else(|| fault("texts"))
                })?;
                Ok(BTreeMap::from([("texts".into(), json!(texts))]))
            })
        }),
    )?;
    crate::ai::workspace_graph::single_operation(
        registry,
        Contract::new("reading.guide-translation", 1),
        operation(),
        inputs(),
        outputs,
    )
}
