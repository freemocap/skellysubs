//! Typed conversational prose boundary. The host supplies current authority,
//! provider admission, persisted wire identities and response/evidence handling.
//! This operation does not choose credentials, publish messages or retry requests.
use super::context;
use crate::ai::{
    connections::access::ResolvedTarget,
    graph::{self, *},
    transport::{graph_text, text_request::TextRequest},
};
use std::{collections::BTreeMap, future::Future, pin::Pin, sync::Arc};

/// Complete through the host's existing transport/batching boundary. The supplied
/// context carries the actual producer identity and its bounded evidence/preview
/// sinks. Before returning a Fault, the host must retain full domain diagnostics;
/// a Fault alone cannot represent a provider response. Observations must already
/// be classified/redacted. A return value is computation, not domain publication.
pub type Provider = Arc<
    dyn Fn(
            InvocationContext,
            Request,
        ) -> Pin<Box<dyn Future<Output = graph::Result<String>> + Send>>
        + Send
        + Sync,
>;

#[derive(Clone)]
pub struct Request {
    pub context: context::Captured,
    pub target: ResolvedTarget,
    pub install_id: String,
    pub temperature: f64,
}

pub(crate) fn prepare_reply(
    completion: &crate::ai::transport::provider::Completion,
    invocation: &InvocationContext,
) -> crate::model::Result<String> {
    use crate::model::{AppError, ErrorCode};
    if completion.finish_reason == "error" {
        return Err(AppError::new(
            ErrorCode::Provider,
            "The provider ended the response with an error.",
        ));
    }
    let mut text = completion.text.clone();
    let mut cleanup = serde_json::json!({});
    if completion.finish_reason == "stop"
        && let Some(clean) = crate::conversations::reply_contract::clean(&text)
    {
        cleanup["role_transcript_cleanup"] =
            serde_json::json!({"removed_bytes":text.len()-clean.len()});
        text = clean.to_owned();
    }
    let (clean, removed) = crate::ai::transport::provider::strip_prose_emojis(&text);
    if removed > 0 && !clean.is_empty() {
        text = clean;
        cleanup["prose_cleanup"] = serde_json::json!({"emoji_graphemes_removed":removed});
    }
    if cleanup.as_object().is_some_and(|m| !m.is_empty()) {
        invocation
            .observe(crate::ai::transport::graph_evidence::metadata(
                &cleanup,
                &[&completion.text],
            ))
            .map_err(super::graph_runtime::error)?;
    }
    crate::conversations::reply_contract::validate(&text)?;
    Ok(text)
}

impl Request {
    /// IDs must be the host's durably captured wire IDs for this producer. They
    /// are not native consumer attempt IDs, and must not be minted on replay.
    pub fn text_request(self, attempt: String, operation: String) -> TextRequest {
        TextRequest {
            decisions: None,
            temperature: self.temperature,
            credential: self.target.credential.clone().unwrap_or_default(),
            model: self.target.model.clone(),
            route: self.target.route,
            target: self.target,
            attempt,
            operation,
            install_id: self.install_id,
            messages: self.context.messages,
        }
    }
}

fn fault(path: &str) -> Fault {
    Fault {
        code: "conversation_prose_input_invalid".into(),
        path: path.into(),
    }
}

fn contract(name: &str) -> Contract {
    Contract::new(name, 1)
}

pub fn operation_contract() -> Contract {
    contract("conversation.generate-prose")
}
pub fn text_contract() -> Contract {
    contract("conversation.generated-text")
}

pub(super) fn inputs() -> Ports {
    let mut ports = graph_text::inputs();
    ports.insert(
        "context".into(),
        Port {
            contract: context::validated_contract(),
            optional: false,
        },
    );
    ports
}

pub(super) fn outputs() -> Ports {
    BTreeMap::from([(
        "text".into(),
        Port {
            contract: text_contract(),
            optional: false,
        },
    )])
}

pub(super) fn decode(values: &Values) -> graph::Result<Request> {
    let settings = graph_text::decode(values).map_err(|cause| fault(&cause.path))?;
    Ok(Request {
        context: serde_json::from_value(
            values
                .get("context")
                .ok_or_else(|| fault("context"))?
                .clone(),
        )
        .map_err(|_| fault("context"))?,
        target: settings.target,
        install_id: settings.install_id,
        temperature: settings.temperature,
    })
}

pub(super) fn capture(
    context: context::Captured,
    target: &ResolvedTarget,
    install_id: &str,
    captured_temperature: f64,
) -> graph::Result<Values> {
    let mut values = graph_text::capture(target, install_id, captured_temperature)
        .map_err(|cause| fault(&cause.path))?;
    values.insert(
        "context".into(),
        serde_json::to_value(context).map_err(|_| fault("context"))?,
    );
    decode(&values)?;
    Ok(values)
}

pub(super) fn register(registry: &mut Registry, provider: Provider) -> graph::Result<()> {
    graph_text::register_types(registry)?;
    registry.define_type(text_contract(), Shape::Text)?;
    registry.register(
        Operation {
            contract: operation_contract(),
            implementation: "conversations/execution/prose/1".into(),
            inputs: inputs(),
            outputs: outputs(),
            resource: Resource::Provider,
            reuse: Reuse::Fresh,
        },
        Arc::new(move |invocation, values| {
            let provider = provider.clone();
            Box::pin(async move {
                let request = decode(&values)?;
                let text = provider(invocation, request).await?;
                Ok(BTreeMap::from([("text".into(), serde_json::json!(text))]))
            })
        }),
    )
}
