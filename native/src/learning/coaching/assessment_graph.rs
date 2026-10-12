//! Captured Jev assessment operation. The same domain validator decides evidence
//! eligibility; graph settlement does not publish learner credit.
mod contracts;
use super::{InputEvidence, assessment_adapter, skill_assessment};
use crate::{
    ai::{
        connections::access::ResolvedTarget,
        graph::{self, *},
        transport::{graph_evidence, graph_text, provider, text_request::TextRequest},
    },
    language::source_graph::{self, SourceText},
    learning::{
        practice_assessment::{Instructions, SkillPrompt},
        turn_assessment::{self, MessageQuestions},
    },
};
use serde::{Deserialize, Serialize};
use std::{collections::BTreeMap, future::Future, pin::Pin, sync::Arc};

#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Context {
    pub messages: Vec<provider::PromptMessage>,
    pub input: InputEvidence,
    pub language: String,
    pub variety: String,
}
#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Content {
    pub skills: Vec<SkillPrompt>,
    pub instructions: Instructions,
    pub questions: MessageQuestions,
}
#[derive(Clone)]
pub struct Request {
    pub source: SourceText,
    pub context: Context,
    pub content: Content,
    pub target: ResolvedTarget,
    pub install_id: String,
    pub temperature: f64,
}
fn fault(path: &str) -> Fault {
    Fault {
        code: "assessment_graph_invalid".into(),
        path: path.into(),
    }
}
impl Request {
    pub fn text_request(
        &self,
        attempt: String,
        operation: String,
    ) -> crate::model::Result<TextRequest> {
        let captured = serde_json::json!({
            "messages":self.context.messages,"input":self.context.input,
            "languageContext":{"target_name":self.context.language,"variety_name":self.context.variety},
        });
        let messages = skill_assessment::prompt_for_source(self.source.text.clone(), &captured)?;
        let state = serde_json::from_str(&messages[1].content)?;
        let decisions = turn_assessment::request(
            state,
            &self.content.skills,
            &self.content.instructions,
            &self.content.questions,
        )?;
        Ok(TextRequest {
            decisions: Some(decisions),
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
pub fn operation_contract() -> Contract {
    Contract::new("learning.assess-message", 1)
}
pub fn context_contract() -> Contract {
    Contract::new("learning.assessment-context", 1)
}
pub fn content_contract() -> Contract {
    Contract::new("learning.assessment-content", 1)
}
pub fn result_contract() -> Contract {
    Contract::new("learning.bound-assessment", 1)
}
fn required(contract: Contract) -> Port {
    Port {
        contract,
        optional: false,
    }
}
pub fn inputs() -> Ports {
    let mut result = graph_text::inputs();
    result.insert("source".into(), required(source_graph::contract()));
    result.insert("context".into(), required(context_contract()));
    result.insert(
        "content".into(),
        Port {
            contract: content_contract(),
            optional: true,
        },
    );
    result
}
pub fn outputs() -> Ports {
    BTreeMap::from([("assessment".into(), required(result_contract()))])
}

pub fn capture(
    source: SourceText,
    context: Context,
    content: Content,
    target: &ResolvedTarget,
    install: &str,
) -> graph::Result<Values> {
    capture_available(source, context, Some(content), target, install)
}
/// Missing captured criteria remain unknown. The assessment invocation fails
/// before provider I/O while independent branches can still execute.
pub fn capture_available(
    source: SourceText,
    context: Context,
    content: Option<Content>,
    target: &ResolvedTarget,
    install: &str,
) -> graph::Result<Values> {
    let mut target = target.clone();
    target.model = assessment_adapter::MODEL.into();
    let mut values = graph_text::capture(
        &target,
        install,
        crate::ai::connections::model_routing::TASK_TEMPERATURE,
    )?;
    values.insert("source".into(), serde_json::json!(source));
    values.insert("context".into(), serde_json::json!(context));
    if let Some(content) = content {
        values.insert("content".into(), serde_json::json!(content));
        decode(&values)?;
    }
    Ok(values)
}
pub fn decode(values: &Values) -> graph::Result<Request> {
    let transport = graph_text::decode(values)?;
    let parse = |name: &str| values.get(name).cloned().ok_or_else(|| fault(name));
    let request = Request {
        source: serde_json::from_value(parse("source")?).map_err(|_| fault("source"))?,
        context: serde_json::from_value(parse("context")?).map_err(|_| fault("context"))?,
        content: serde_json::from_value(parse("content")?).map_err(|_| fault("content"))?,
        target: transport.target,
        install_id: transport.install_id,
        temperature: transport.temperature,
    };
    if request.source.id.is_empty()
        || request.source.text.trim().is_empty()
        || request.target.model != assessment_adapter::MODEL
        || !request
            .context
            .messages
            .last()
            .is_some_and(|m| m.role == "user" && m.content == request.source.text)
    {
        return Err(fault("source_or_model"));
    }
    request
        .text_request(String::new(), String::new())
        .map_err(|_| fault("request"))?;
    Ok(request)
}
/// Shared transport/source contracts are registered by the enclosing graph.
pub fn register(registry: &mut Registry, provider: Provider) -> graph::Result<()> {
    contracts::register(registry)?;
    registry.register(
        Operation {
            contract: operation_contract(),
            implementation: "learning/assessment/graph/1".into(),
            inputs: inputs(),
            outputs: outputs(),
            resource: Resource::Provider,
            reuse: Reuse::Exact,
        },
        Arc::new(move |invocation, values| {
            let provider = provider.clone();
            Box::pin(async move {
                let request = decode(&values).map_err(|failure| {
                    invocation.observe(graph_evidence::metadata(&serde_json::json!({"stage":"assessment_input_validation","code":failure.code,"path":failure.path,"message":"Captured assessment inputs are unavailable or invalid."}), &[])).err().unwrap_or(failure)
                })?;
                let completion = provider(invocation.clone(), request.clone()).await?;
                let assessment = assessment_adapter::validate_captured(
                    &completion,
                    &request.content.skills,
                    &request.content.instructions,
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
                Ok(BTreeMap::from([(
                    "assessment".into(),
                    serde_json::json!({"source":request.source,"assessment":assessment}),
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
        contract: Contract::new("learning.message-assessment", 1),
        inputs: inputs(),
        outputs: outputs(),
        nodes: BTreeMap::from([(
            "assessment".into(),
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
            "assessment".into(),
            Source::Output {
                node: "assessment".into(),
                port: "assessment".into(),
            },
        )]),
        compositions: BTreeMap::new(),
    })
}

#[cfg(test)]
mod tests;
