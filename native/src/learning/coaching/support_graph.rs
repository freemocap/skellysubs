//! Typed reply brief and draft-assistance operations. Prompt construction and
//! validation use the shared domain functions; no database lookup occurs here.
mod contracts;
pub mod explanation;
pub mod explanation_snapshot;
use super::{InputEvidence, conversation_support as support};
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
pub struct Language {
    pub script: String,
    pub guidance: BTreeMap<String, Vec<String>>,
}
#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Practice {
    pub difficulty: String,
}
#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Context {
    pub messages: Vec<provider::PromptMessage>,
    pub target_language: String,
    pub translation_language: String,
    pub language_context: Language,
    pub practice_settings: Practice,
    pub input: Option<InputEvidence>,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Task {
    Brief,
    Assistance,
}
impl Task {
    pub fn kind(self) -> &'static str {
        match self {
            Self::Brief => support::BRIEF,
            Self::Assistance => support::ASSISTANCE,
        }
    }
    pub fn operation(self) -> Contract {
        Contract::new(
            match self {
                Self::Brief => "conversation.explain-reply",
                Self::Assistance => "conversation.draft-replies",
            },
            1,
        )
    }
    pub fn result(self) -> Contract {
        Contract::new(
            match self {
                Self::Brief => "conversation.bound-reply-brief",
                Self::Assistance => "conversation.bound-draft-replies",
            },
            1,
        )
    }
}
#[derive(Clone)]
pub struct Request {
    pub task: Task,
    pub source: SourceText,
    pub learner: Option<SourceText>,
    pub context: Context,
    pub target: ResolvedTarget,
    pub install_id: String,
    pub temperature: f64,
}
impl Request {
    pub fn schema(&self) -> serde_json::Value {
        support::schema_for_context(self.task.kind(), &serde_json::json!(self.context))
    }
    pub fn text_request(
        &self,
        attempt: String,
        operation: String,
    ) -> crate::model::Result<TextRequest> {
        let messages = support::prompt_for_exchange(
            self.source.text.clone(),
            self.learner.as_ref().map(|s| s.text.clone()),
            self.task.kind(),
            &serde_json::json!(self.context),
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
        code: "support_graph_invalid".into(),
        path: path.into(),
    }
}
pub fn context_contract() -> Contract {
    Contract::new("conversation.support-context", 1)
}
fn required(contract: Contract) -> Port {
    Port {
        contract,
        optional: false,
    }
}
pub fn inputs() -> Ports {
    let mut inputs = graph_text::inputs();
    inputs.insert("source".into(), required(source_graph::contract()));
    inputs.insert(
        "learner".into(),
        Port {
            contract: source_graph::contract(),
            optional: true,
        },
    );
    inputs.insert("context".into(), required(context_contract()));
    inputs
}
pub fn capture(
    task: Task,
    source: SourceText,
    learner: Option<SourceText>,
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
    if let Some(learner) = learner {
        values.insert("learner".into(), serde_json::json!(learner));
    }
    decode(task, &values)?;
    Ok(values)
}
pub fn decode(task: Task, values: &Values) -> graph::Result<Request> {
    let request = decode_inputs(task, values)?;
    request
        .text_request(String::new(), String::new())
        .map_err(|_| fault("prompt"))?;
    Ok(request)
}
fn decode_inputs(task: Task, values: &Values) -> graph::Result<Request> {
    let settings = graph_text::decode(values)?;
    let request = Request {
        task,
        source: serde_json::from_value(
            values
                .get("source")
                .cloned()
                .ok_or_else(|| fault("source"))?,
        )
        .map_err(|_| fault("source"))?,
        learner: values
            .get("learner")
            .cloned()
            .map(serde_json::from_value)
            .transpose()
            .map_err(|_| fault("learner"))?,
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
        || request.learner.as_ref().is_some_and(|s| {
            s.id.is_empty()
                || s.id == request.source.id
                || !request
                    .context
                    .messages
                    .last()
                    .is_some_and(|m| m.role == "user" && m.content == s.text)
        })
    {
        return Err(fault("source"));
    }
    Ok(request)
}

pub fn register(registry: &mut Registry, provider: Provider) -> graph::Result<()> {
    contracts::register(registry)?;
    for task in [Task::Brief, Task::Assistance] {
        let provider = provider.clone();
        registry.register(
            Operation {
                contract: task.operation(),
                implementation: format!("learning/support/{}/graph/1", task.kind()),
                inputs: inputs(),
                outputs: BTreeMap::from([("support".into(), required(task.result()))]),
                resource: Resource::Provider,
                reuse: Reuse::Exact,
            },
            Arc::new(move |invocation, values| {
                let provider = provider.clone();
                Box::pin(async move {
                    let request = decode(task, &values)?;
                    let completion = provider(invocation.clone(), request.clone()).await?;
                    let value = support::validate(task.kind(), &completion).map_err(|error| {
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
                        "support".into(),
                        serde_json::json!({"source":request.source,"value":value}),
                    )]))
                })
            }),
        )?;
    }
    Ok(())
}

#[cfg(test)]
mod tests;
