//! Source-bound reading explanation/completion. Shared support prompts and
//! validators retain sentence-template semantics without conversation ownership.
use super::{sentence_blanks, support};
use crate::{
    ai::{
        graph::{self, *},
        transport::{graph_evidence, graph_text, provider, text_request::TextRequest},
    },
    language::source_graph::{self, SourceText},
};
use serde::{Deserialize, Serialize};
use std::{collections::BTreeMap, future::Future, pin::Pin, sync::Arc};

#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Context {
    pub target: String,
    pub explanation: String,
    pub script: String,
    pub guidance: BTreeMap<String, Vec<String>>,
    pub template: bool,
}
#[derive(Clone)]
pub struct Request {
    pub source: SourceText,
    pub context: Context,
    settings: graph_text::Settings,
}
impl Request {
    pub fn prepare(
        &self,
        attempt: String,
        operation: String,
    ) -> crate::model::Result<(TextRequest, serde_json::Value)> {
        let captured = serde_json::json!({"targetLanguage":self.context.target,"translationLanguage":self.context.explanation,"languageContext":{"script":self.context.script,"guidance":self.context.guidance},"messages":[]});
        let mut schema = support::schema_for_context(support::EXPLANATIONS, &captured);
        let mut messages = support::prompt_for_exchange(
            self.source.text.clone(),
            None,
            support::EXPLANATIONS,
            &captured,
        )?;
        if sentence_blanks::contains(&self.source.text, self.context.template) {
            schema["properties"]["cards"]["minItems"] = serde_json::json!(2);
            schema["properties"]["cards"]["maxItems"] = serde_json::json!(3);
            messages[0].content.push('\n');
            messages[0].content.push_str(sentence_blanks::INSTRUCTION);
        }
        Ok((
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
                messages,
            },
            schema,
        ))
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
    Contract::new("reading.explain-selection", 1)
}
fn context_contract() -> Contract {
    Contract::new("reading.explanation-context", 1)
}
fn result_contract() -> Contract {
    Contract::new("reading.bound-explanation", 1)
}
fn port(contract: Contract) -> Port {
    Port {
        contract,
        optional: false,
    }
}
fn fault(path: &str) -> Fault {
    Fault {
        code: "reading_explanation_invalid".into(),
        path: path.into(),
    }
}
fn inputs() -> Ports {
    let mut ports = graph_text::inputs();
    ports.insert("source".into(), port(source_graph::contract()));
    ports.insert("context".into(), port(context_contract()));
    ports
}
pub fn capture(
    source: SourceText,
    context: Context,
    target: &crate::ai::connections::access::ResolvedTarget,
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
    let source: SourceText = serde_json::from_value(
        values
            .get("source")
            .cloned()
            .ok_or_else(|| fault("source"))?,
    )
    .map_err(|_| fault("source"))?;
    let context: Context = serde_json::from_value(
        values
            .get("context")
            .cloned()
            .ok_or_else(|| fault("context"))?,
    )
    .map_err(|_| fault("context"))?;
    if source.id.is_empty()
        || source.text.trim().is_empty()
        || context.target.is_empty()
        || context.explanation.is_empty()
    {
        return Err(fault("source"));
    }
    Ok(Request {
        source,
        context,
        settings,
    })
}
fn record(fields: impl IntoIterator<Item = (&'static str, Shape)>) -> Shape {
    Shape::Record(
        fields
            .into_iter()
            .map(|(key, shape)| (key.into(), shape))
            .collect(),
    )
}
pub fn compile(provider: Provider) -> graph::Result<Executable> {
    let mut registry = Registry::default();
    graph_text::register_types(&mut registry)?;
    source_graph::register(&mut registry)?;
    registry.define_type(
        context_contract(),
        record([
            ("target", Shape::Text),
            ("explanation", Shape::Text),
            ("script", Shape::Text),
            (
                "guidance",
                Shape::Map(Box::new(Shape::List(Box::new(Shape::Text)))),
            ),
            ("template", Shape::Boolean),
        ]),
    )?;
    registry.define_type(
        result_contract(),
        record([
            ("source", source_graph::shape()),
            (
                "value",
                record([(
                    "cards",
                    Shape::List(Box::new(record([
                        ("quote", Shape::Text),
                        ("title", Shape::Text),
                        ("body", Shape::Text),
                        ("example", Shape::Text),
                        ("contrast", Shape::Text),
                    ]))),
                )]),
            ),
        ]),
    )?;
    let outputs = BTreeMap::from([("explanations".into(), port(result_contract()))]);
    registry.register(
        Operation {
            contract: operation(),
            implementation: "reading/explanations/1".into(),
            inputs: inputs(),
            outputs: outputs.clone(),
            resource: Resource::Provider,
            reuse: Reuse::Exact,
        },
        Arc::new(move |invocation, values| {
            let provider = provider.clone();
            Box::pin(async move {
                let request = decode(&values)?;
                let completion = provider(invocation.clone(), request.clone()).await?;
                let validate = || -> crate::model::Result<support::ReplyExplanations> {
                    let result = serde_json::from_value(support::validate(
                        support::EXPLANATIONS,
                        &completion,
                    )?)?;
                    sentence_blanks::validate(
                        &request.source.text,
                        request.context.template,
                        &result,
                    )?;
                    Ok(result)
                };
                let result = validate().map_err(|error| {
                    invocation
                        .observe(graph_evidence::failure(
                            &error,
                            &request.settings.target.model,
                            &[&request.source.text, &completion.text],
                        ))
                        .err()
                        .unwrap_or_else(|| fault("response"))
                })?;
                Ok(BTreeMap::from([(
                    "explanations".into(),
                    serde_json::json!({"source":request.source,"value":result}),
                )]))
            })
        }),
    )?;
    let graph = crate::ai::workspace_graph::single_operation(
        registry,
        Contract::new("reading.selection-help", 1),
        operation(),
        inputs(),
        outputs,
    )?;
    Ok(graph)
}
