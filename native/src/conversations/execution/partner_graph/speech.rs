//! The same playback composition serves automatic reply speech and later
//! requested playback. Missing captured routing is not a fabricated target.
use super::*;
use crate::speech::synthesis_graph::{self as synthesis, playback};

pub(super) fn extend(
    registry: &mut Registry,
    definition: &mut Definition,
    provider: synthesis::Provider,
    lookup: playback::Lookup,
) -> Result<()> {
    synthesis::register_types(registry)?;
    synthesis::register(registry, provider)?;
    playback::register(registry, lookup)?;
    let child = registry.compile(playback::definition())?;
    definition.inputs.insert(
        "speech".into(),
        synthesis::inputs().remove("speech").unwrap(),
    );
    let results = definition.compose(
        "speech",
        &child,
        BTreeMap::from([
            (
                "source".into(),
                Source::Output {
                    node: "reply_source".into(),
                    port: "source".into(),
                },
            ),
            ("speech".into(), Source::Input("speech".into())),
            (
                "regenerate".into(),
                Source::Constant {
                    contract: playback::definition().inputs["regenerate"].contract.clone(),
                    value: serde_json::json!(false),
                },
            ),
        ]),
    )?;
    definition.outputs.insert(
        "audio".into(),
        Port {
            contract: synthesis::result_contract(),
            optional: true,
        },
    );
    definition
        .results
        .insert("audio".into(), results["audio"].clone());
    Ok(())
}
