//! Source-bound synthesis. The host retains response evidence and atomically
//! stores a receipt with settlement; graph history contains no audio payload.
use crate::{
    ai::{
        audio,
        connections::access::ResolvedTarget,
        graph::{self, *},
    },
    language::source_graph::{self, SourceText},
};
use serde::{Deserialize, Serialize};
use std::{collections::BTreeMap, future::Future, pin::Pin, sync::Arc};

mod capture;
pub mod playback;
pub use capture::{capture, capture_settings, decode, inputs, register_types};
#[cfg(test)]
mod tests;

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct Settings {
    pub target: ResolvedTarget,
    pub install_id: String,
    pub language_tag: String,
    pub language: String,
    pub voice: String,
}

#[derive(Clone)]
pub struct Request {
    pub source: SourceText,
    pub settings: Settings,
}
impl Request {
    pub fn speech_input(&self) -> audio::SpeechInput {
        audio::SpeechInput {
            text: self.source.text.clone(),
            language_tag: self.settings.language_tag.clone(),
            language: self.settings.language.clone(),
            voice: self.settings.voice.clone(),
        }
    }
}

/// The referenced receipt owns alignment and diagnostics independently of its
/// evictable payload. The host must reject a missing or mismatched receipt during
/// settlement/adoption; an operation callback cannot establish durable storage.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct Receipt {
    pub id: String,
    pub engine: String,
    pub execution: String,
    pub digest: String,
    pub bytes: u64,
}
pub type Provider = Arc<
    dyn Fn(
            InvocationContext,
            Request,
        ) -> Pin<Box<dyn Future<Output = graph::Result<Receipt>> + Send>>
        + Send
        + Sync,
>;

fn fault(path: &str) -> Fault {
    Fault {
        code: "speech_graph_input_invalid".into(),
        path: path.into(),
    }
}
fn contract(name: &str) -> Contract {
    Contract::new(name, 1)
}
pub fn operation_contract() -> Contract {
    contract("speech.synthesize")
}
pub fn result_contract() -> Contract {
    contract("speech.bound-audio-receipt")
}
fn required(contract: Contract) -> Port {
    Port {
        contract,
        optional: false,
    }
}
pub fn outputs() -> Ports {
    BTreeMap::from([("audio".into(), required(result_contract()))])
}
pub fn register(registry: &mut Registry, provider: Provider) -> graph::Result<()> {
    registry.define_type(
        result_contract(),
        Shape::Record(BTreeMap::from([
            ("source".into(), source_graph::shape()),
            (
                "receipt".into(),
                Shape::Record(BTreeMap::from([
                    ("id".into(), Shape::Text),
                    ("engine".into(), Shape::Text),
                    ("execution".into(), Shape::Text),
                    ("digest".into(), Shape::Text),
                    ("bytes".into(), Shape::Integer),
                ])),
            ),
        ])),
    )?;
    registry.register(
        Operation {
            contract: operation_contract(),
            implementation: "speech/synthesis/graph/1".into(),
            inputs: inputs(),
            outputs: outputs(),
            resource: Resource::Provider,
            // Replaying an old graph output cannot establish that evictable audio
            // still exists. Requested playback performs an explicit cache lookup.
            reuse: Reuse::Fresh,
        },
        Arc::new(move |invocation, values| {
            let provider = provider.clone();
            Box::pin(async move {
                let request = decode(&values)?;
                let source = request.source.clone();
                let receipt = provider(invocation.clone(), request).await?;
                let execution = serde_json::to_string(&invocation.identity().execution)
                    .map_err(|_| fault("execution"))?;
                if receipt.id.is_empty()
                    || receipt.engine.is_empty()
                    || invocation.identity().engine.as_deref() != Some(receipt.engine.as_str())
                    || execution != receipt.execution
                    || receipt.bytes == 0
                    || receipt.bytes > super::delivery::AUDIO_LIMIT as u64
                    || receipt.digest.len() != 64
                    || !receipt
                        .digest
                        .bytes()
                        .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
                {
                    return Err(Fault {
                        code: "speech_receipt_invalid".into(),
                        path: "audio.receipt".into(),
                    });
                }
                Ok(BTreeMap::from([(
                    "audio".into(),
                    serde_json::json!({"source":source,"receipt":receipt}),
                )]))
            })
        }),
    )
}

pub fn compile(provider: Provider) -> graph::Result<Executable> {
    let mut registry = Registry::default();
    source_graph::register(&mut registry)?;
    register_types(&mut registry)?;
    register(&mut registry, provider)?;
    registry.compile(Definition {
        contract: contract("speech.requested-synthesis"),
        inputs: inputs(),
        outputs: outputs(),
        nodes: BTreeMap::from([(
            "synthesize".into(),
            Node {
                operation: operation_contract(),
                inputs: inputs()
                    .keys()
                    .map(|key| (key.clone(), Source::Input(key.clone())))
                    .collect(),
                after: vec![],
                guard: None,
                activation: Activation::OnDemand,
            },
        )]),
        results: BTreeMap::from([(
            "audio".into(),
            Source::Output {
                node: "synthesize".into(),
                port: "audio".into(),
            },
        )]),
        compositions: BTreeMap::new(),
    })
}
