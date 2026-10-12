//! Typed recording recognition. Graph history stores a digest/reference, never
//! a permanent audio copy. The host resolves the captured recording at dispatch.
use crate::ai::{
    audio,
    connections::access::ResolvedTarget,
    graph::{self, *},
    transport::graph_audio_target,
};
use serde_json::{Value, json};
use std::{collections::BTreeMap, future::Future, pin::Pin, sync::Arc};
pub type Provider = Arc<
    dyn Fn(
            InvocationContext,
            Values,
        )
            -> Pin<Box<dyn Future<Output = graph::Result<audio::TranscriptionResult>> + Send>>
        + Send
        + Sync,
>;
fn contract(name: &str) -> Contract {
    Contract::new(name, 1)
}
pub fn operation() -> Contract {
    contract("speech.transcribe-recording")
}
fn port(name: &str) -> Port {
    Port {
        contract: contract(name),
        optional: false,
    }
}
fn inputs() -> Ports {
    BTreeMap::from([("request".into(), port("speech.captured-recording"))])
}
fn outputs() -> Ports {
    BTreeMap::from([("transcript".into(), port("speech.transcript"))])
}
pub fn capture(
    target: &ResolvedTarget,
    input: &audio::TranscriptionRequest,
    install: &str,
) -> crate::model::Result<Values> {
    audio::validate_transcription_language(target, &input.language)?;
    if install.is_empty() || input.wav.is_empty() || input.wav.len() > 25 * 1024 * 1024 {
        return Err(crate::model::AppError::new(
            crate::model::ErrorCode::Validation,
            "Recording must contain audio and be at most 25 MB.",
        ));
    }
    Ok(BTreeMap::from([(
        "request".into(),
        json!({"target":graph_audio_target::capture(target)?,"install":install,"digest":crate::ai::results::digest(&input.wav),"bytes":input.wav.len(),"language":{"id":input.language.language_id,"variety":input.language.variety_id,"tag":input.language.language_tag},"context":input.context}),
    )]))
}
pub fn compile(provider: Provider) -> graph::Result<Executable> {
    let mut registry = Registry::default();
    registry.define_type(
        contract("speech.captured-recording"),
        Shape::Record(BTreeMap::from([
            ("target".into(), graph_audio_target::shape()),
            ("install".into(), Shape::Text),
            ("digest".into(), Shape::Text),
            ("bytes".into(), Shape::Integer),
            (
                "language".into(),
                Shape::Record(
                    ["id", "variety", "tag"]
                        .into_iter()
                        .map(|k| (k.into(), Shape::Text))
                        .collect(),
                ),
            ),
            ("context".into(), Shape::Nullable(Box::new(Shape::Text))),
        ])),
    )?;
    registry.define_type(
        contract("speech.transcript"),
        Shape::Record(BTreeMap::from([
            ("text".into(), Shape::Text),
            (
                "timing".into(),
                Shape::Nullable(Box::new(Shape::Record(BTreeMap::from([
                    ("text".into(), Shape::Text),
                    ("duration".into(), Shape::Number),
                    (
                        "words".into(),
                        Shape::List(Box::new(Shape::Record(BTreeMap::from([
                            ("word".into(), Shape::Text),
                            ("start".into(), Shape::Number),
                            ("end".into(), Shape::Number),
                        ])))),
                    ),
                ])))),
            ),
        ])),
    )?;
    registry.register(
        Operation {
            contract: operation(),
            implementation: "speech/transcription/graph/1".into(),
            inputs: inputs(),
            outputs: outputs(),
            resource: Resource::Provider,
            reuse: Reuse::Exact,
        },
        Arc::new(move |context, values| {
            let provider = provider.clone();
            Box::pin(async move {
                let result = provider(context, values).await?;
                let value: Value = serde_json::to_value(result).map_err(|_| Fault {
                    code: "transcription_output_invalid".into(),
                    path: "transcript".into(),
                })?;
                Ok(BTreeMap::from([("transcript".into(), value)]))
            })
        }),
    )?;
    crate::ai::workspace_graph::single_operation(
        registry,
        contract("speech.recording-recognition"),
        operation(),
        inputs(),
        outputs(),
    )
}
