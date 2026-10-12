//! Fresh structured proposal requests. Captured prompts are source inputs;
//! the native output is a candidate, whose product acceptance remains separate.
use crate::ai::{
    graph::{self, *},
    transport::{graph_evidence, graph_text, provider, text_request::TextRequest},
};
use serde_json::{Value, json};
use std::{collections::BTreeMap, future::Future, pin::Pin, sync::Arc};

#[derive(Clone, Copy)]
pub enum Kind {
    Persona,
    Drill,
}
impl Kind {
    pub fn operation(self) -> Contract {
        Contract::new(
            match self {
                Self::Persona => "partner.propose-persona",
                Self::Drill => "drill.propose-items",
            },
            1,
        )
    }
    fn result(self) -> Contract {
        Contract::new(
            match self {
                Self::Persona => "partner.persona-candidate",
                Self::Drill => "drill.item-candidates",
            },
            1,
        )
    }
}
#[derive(Clone)]
pub struct Request {
    pub kind: Kind,
    pub messages: Vec<provider::PromptMessage>,
    pub max_output_tokens: i32,
    pub romanized: bool,
    pub length: crate::drill::generation::DrillLength,
    settings: graph_text::Settings,
}
impl Request {
    pub fn prepare(
        &self,
        attempt: String,
        operation: String,
    ) -> (TextRequest, Value, &'static str) {
        let (schema, name) = match self.kind {
            Kind::Persona => (
                crate::partners::persona::generation_schema_for_romanization(self.romanized),
                crate::partners::persona::persona_prompt::SCHEMA_NAME,
            ),
            Kind::Drill => (
                crate::drill::generation::schema(self.length),
                "drill_candidates",
            ),
        };
        (
            TextRequest {
                decisions: None,
                temperature: self.settings.temperature,
                target: self.settings.target.clone(),
                credential: self.settings.target.credential.clone().unwrap_or_default(),
                model: self.settings.target.model.clone(),
                route: self.settings.target.route,
                install_id: self.settings.install_id.clone(),
                attempt,
                operation,
                messages: self.messages.clone(),
            },
            schema,
            name,
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
fn fault(path: &str) -> Fault {
    Fault {
        code: "proposal_graph_invalid".into(),
        path: path.into(),
    }
}
fn required(name: &str) -> Port {
    Port {
        contract: Contract::new(name, 1),
        optional: false,
    }
}
fn inputs() -> Ports {
    let mut ports = graph_text::inputs();
    for (port, name) in [
        ("owner", "ai.proposal-owner"),
        ("messages", "ai.proposal-messages"),
        ("tokens", "ai.proposal-token-limit"),
        ("romanized", "ai.proposal-romanization"),
        ("length", "ai.proposal-item-length"),
    ] {
        ports.insert(port.into(), required(name));
    }
    ports
}
pub fn capture(
    kind: Kind,
    request: &super::Request,
    messages: Vec<provider::PromptMessage>,
    tokens: i32,
    length: crate::drill::generation::DrillLength,
) -> graph::Result<Values> {
    let mut values = graph_text::capture(
        &request.target,
        &request.install_id,
        crate::ai::connections::model_routing::TASK_TEMPERATURE,
    )?;
    values.insert("owner".into(), json!(request.id));
    values.insert("messages".into(), json!(messages));
    values.insert("tokens".into(), json!(tokens));
    values.insert(
        "romanized".into(),
        json!(request.language.romanization.is_some()),
    );
    values.insert("length".into(), json!(length));
    decode(kind, &values)?;
    Ok(values)
}
pub fn decode(kind: Kind, values: &Values) -> graph::Result<Request> {
    let settings = graph_text::decode(values)?;
    let get = |key: &str| values.get(key).cloned().ok_or_else(|| fault(key));
    let messages: Vec<provider::PromptMessage> =
        serde_json::from_value(get("messages")?).map_err(|_| fault("messages"))?;
    let max_output_tokens = serde_json::from_value(get("tokens")?).map_err(|_| fault("tokens"))?;
    if !(1..=32768).contains(&max_output_tokens) || messages.is_empty() {
        return Err(fault("request"));
    }
    if get("owner")?.as_str().is_none_or(str::is_empty) {
        return Err(fault("owner"));
    }
    Ok(Request {
        kind,
        settings,
        messages,
        max_output_tokens,
        romanized: serde_json::from_value(get("romanized")?).map_err(|_| fault("romanized"))?,
        length: serde_json::from_value(get("length")?).map_err(|_| fault("length"))?,
    })
}
/// Structural translation only. Domain bounds/acceptance remain with the domain
/// validator; unsupported schema structures fail rather than becoming untyped JSON.
fn shape(schema: &Value) -> graph::Result<Shape> {
    if let Some(types) = schema["type"].as_array() {
        if types.len() != 2 || !types.contains(&json!("null")) {
            return Err(fault("schema"));
        }
        let mut inner = schema.clone();
        inner["type"] = types
            .iter()
            .find(|v| **v != json!("null"))
            .cloned()
            .ok_or_else(|| fault("schema"))?;
        return Ok(Shape::Nullable(Box::new(shape(&inner)?)));
    }
    Ok(match schema["type"].as_str() {
        Some("string") => Shape::Text,
        Some("integer") => Shape::Integer,
        Some("array") => Shape::List(Box::new(shape(&schema["items"])?)),
        Some("object") if schema["additionalProperties"] == false => {
            let properties = schema["properties"]
                .as_object()
                .ok_or_else(|| fault("schema"))?;
            let required = schema["required"]
                .as_array()
                .ok_or_else(|| fault("schema"))?;
            if required.len() != properties.len()
                || properties.keys().any(|key| !required.contains(&json!(key)))
            {
                return Err(fault("schema"));
            }
            Shape::Record(
                properties
                    .iter()
                    .map(|(key, value)| Ok((key.clone(), shape(value)?)))
                    .collect::<graph::Result<_>>()?,
            )
        }
        _ => return Err(fault("schema")),
    })
}
pub fn compile(kind: Kind, provider: Provider) -> graph::Result<Executable> {
    let mut registry = Registry::default();
    graph_text::register_types(&mut registry)?;
    for (name, shape) in [
        ("ai.proposal-owner", Shape::Text),
        (
            "ai.proposal-messages",
            Shape::List(Box::new(Shape::Record(BTreeMap::from([
                ("role".into(), Shape::Text),
                ("content".into(), Shape::Text),
            ])))),
        ),
        ("ai.proposal-token-limit", Shape::Integer),
        ("ai.proposal-romanization", Shape::Boolean),
        ("ai.proposal-item-length", Shape::Text),
    ] {
        registry.define_type(Contract::new(name, 1), shape)?;
    }
    let schema = match kind {
        Kind::Persona => crate::partners::persona::output_schema(),
        Kind::Drill => {
            crate::drill::generation::schema(crate::drill::generation::DrillLength::Sentence)
        }
    };
    registry.define_type(kind.result(), shape(&schema)?)?;
    let outputs = BTreeMap::from([(
        "candidate".into(),
        Port {
            contract: kind.result(),
            optional: false,
        },
    )]);
    registry.register(
        Operation {
            contract: kind.operation(),
            implementation: "proposals/structured-candidates/1".into(),
            inputs: inputs(),
            outputs: outputs.clone(),
            resource: Resource::Provider,
            reuse: Reuse::Fresh,
        },
        Arc::new(move |invocation, values| {
            let provider = provider.clone();
            Box::pin(async move {
                let request = decode(kind, &values)?;
                let completion = provider(invocation.clone(), request.clone()).await?;
                if completion.finish_reason == "error" {
                    let error = crate::model::AppError::new(crate::model::ErrorCode::Provider, "The AI service did not finish the generation. No automatic retry was made.");
                    return Err(invocation.observe(graph_evidence::failure(&error, &request.settings.target.model, &[&completion.text])).err().unwrap_or_else(|| fault("provider_completion")));
                }
                let value: Value = serde_json::from_str(&completion.text).map_err(|cause| {
                    let error = crate::diagnostics::response::json_context(
                        &cause,
                        "proposal_candidate_decode",
                        crate::model::AppError::new(
                            crate::model::ErrorCode::Validation,
                            "Proposal response is invalid.",
                        ),
                    );
                    invocation
                        .observe(graph_evidence::failure(
                            &error,
                            &request.settings.target.model,
                            &[&completion.text],
                        ))
                        .err()
                        .unwrap_or_else(|| fault("candidate"))
                })?;
                Ok(BTreeMap::from([("candidate".into(), value)]))
            })
        }),
    )?;
    crate::ai::workspace_graph::single_operation(
        registry,
        Contract::new(
            match kind {
                Kind::Persona => "partner.persona-proposal",
                Kind::Drill => "drill.item-proposal",
            },
            1,
        ),
        kind.operation(),
        inputs(),
        outputs,
    )
}
