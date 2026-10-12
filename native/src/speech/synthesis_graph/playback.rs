//! Explicit playback composition: fresh cache lookup, guarded synthesis and a
//! receipt selection. Cache presence never changes the artifact's topology.
use super::*;

pub type Lookup = Arc<
    dyn Fn(
            InvocationContext,
            Request,
        ) -> Pin<Box<dyn Future<Output = graph::Result<Option<Receipt>>> + Send>>
        + Send
        + Sync,
>;
pub fn lookup_operation() -> Contract {
    contract("speech.lookup-audio")
}
pub fn select_operation() -> Contract {
    contract("speech.select-audio")
}
fn boolean() -> Contract {
    contract("speech.regeneration-request")
}
fn optional_audio() -> Port {
    Port {
        contract: result_contract(),
        optional: true,
    }
}
fn lookup_inputs() -> Ports {
    let mut ports = inputs();
    ports.insert("regenerate".into(), required(boolean()));
    ports
}
pub fn register(registry: &mut Registry, lookup: Lookup) -> graph::Result<()> {
    registry.define_type(boolean(), Shape::Boolean)?;
    registry.register(
        Operation {
            contract: lookup_operation(),
            implementation: "speech/cache-lookup/graph/1".into(),
            inputs: lookup_inputs(),
            outputs: BTreeMap::from([
                ("cached".into(), optional_audio()),
                ("generate".into(), required(boolean())),
            ]),
            resource: Resource::Local,
            reuse: Reuse::Fresh,
        },
        Arc::new(move |invocation, values| {
            let lookup = lookup.clone();
            Box::pin(async move {
                let request = decode(&values)?;
                let regenerate = values
                    .get("regenerate")
                    .and_then(serde_json::Value::as_bool)
                    .ok_or_else(|| fault("regenerate"))?;
                let source = request.source.clone();
                let cached = if regenerate {
                    None
                } else {
                    lookup(invocation, request).await?
                };
                let mut output =
                    BTreeMap::from([("generate".into(), serde_json::json!(cached.is_none()))]);
                if let Some(receipt) = cached {
                    // The lookup host verifies storage integrity and pins bounded
                    // delivery; cached provenance remains the original producer.
                    output.insert(
                        "cached".into(),
                        serde_json::json!({"source":source,"receipt":receipt}),
                    );
                }
                Ok(output)
            })
        }),
    )?;
    registry.register(
        Operation {
            contract: select_operation(),
            implementation: "speech/receipt-selection/graph/1".into(),
            inputs: BTreeMap::from([
                ("source".into(), required(source_graph::contract())),
                ("cached".into(), optional_audio()),
                ("generated".into(), optional_audio()),
            ]),
            outputs: outputs(),
            resource: Resource::Local,
            reuse: Reuse::Fresh,
        },
        Arc::new(|_, values| {
            Box::pin(async move {
                let value = match (values.get("cached"), values.get("generated")) {
                    (Some(value), None) | (None, Some(value)) => value,
                    _ => return Err(fault("audio.selection")),
                };
                if value.get("source") != values.get("source") {
                    return Err(fault("audio.source"));
                }
                Ok(BTreeMap::from([("audio".into(), value.clone())]))
            })
        }),
    )
}
fn output(node: &str, port: &str) -> Source {
    Source::Output {
        node: node.into(),
        port: port.into(),
    }
}
/// Only lookup requires demand. Its actual result authorizes the synthesis
/// branch. Explicit regeneration bypasses lookup; it is not an automatic retry.
pub fn definition() -> Definition {
    let bindings = || {
        inputs()
            .keys()
            .map(|key| (key.clone(), Source::Input(key.clone())))
            .collect()
    };
    Definition {
        contract: contract("speech.requested-playback"),
        inputs: lookup_inputs(),
        outputs: outputs(),
        nodes: BTreeMap::from([
            (
                "lookup".into(),
                Node {
                    operation: lookup_operation(),
                    inputs: lookup_inputs()
                        .keys()
                        .map(|key| (key.clone(), Source::Input(key.clone())))
                        .collect(),
                    after: vec![],
                    guard: None,
                    activation: Activation::OnDemand,
                },
            ),
            (
                "synthesize".into(),
                Node {
                    operation: operation_contract(),
                    inputs: bindings(),
                    after: vec![],
                    guard: Some(output("lookup", "generate")),
                    activation: Activation::Automatic,
                },
            ),
            (
                "audio".into(),
                Node {
                    operation: select_operation(),
                    inputs: BTreeMap::from([
                        ("source".into(), Source::Input("source".into())),
                        ("cached".into(), output("lookup", "cached")),
                        ("generated".into(), output("synthesize", "audio")),
                    ]),
                    after: vec![],
                    guard: None,
                    activation: Activation::Automatic,
                },
            ),
        ]),
        results: BTreeMap::from([("audio".into(), output("audio", "audio"))]),
        compositions: BTreeMap::new(),
    }
}
pub fn compile(provider: Provider, lookup: Lookup) -> graph::Result<Executable> {
    let mut registry = Registry::default();
    source_graph::register(&mut registry)?;
    register_types(&mut registry)?;
    super::register(&mut registry, provider)?;
    register(&mut registry, lookup)?;
    registry.compile(definition())
}

#[cfg(test)]
mod tests;
