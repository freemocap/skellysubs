use super::*;

pub fn inputs() -> Ports {
    BTreeMap::from([
        ("source".into(), required(source_graph::contract())),
        (
            "speech".into(),
            // Disabled read-aloud has no captured speech route. Absence keeps
            // the node in the graph; invocation still requires real settings.
            Port {
                contract: contract("speech.captured-settings"),
                optional: true,
            },
        ),
    ])
}
pub fn capture(source: SourceText, settings: Settings) -> graph::Result<Values> {
    let serialized = capture_settings(settings)?;
    let values = BTreeMap::from([
        (
            "source".into(),
            serde_json::to_value(source).map_err(|_| fault("source"))?,
        ),
        ("speech".into(), serialized),
    ]);
    decode(&values)?;
    Ok(values)
}
pub fn capture_settings(settings: Settings) -> graph::Result<serde_json::Value> {
    let mut serialized = serde_json::to_value(settings).map_err(|_| fault("speech"))?;
    if serialized["target"].get("audio_resolution").is_none() {
        serialized["target"]["audio_resolution"] = serde_json::Value::Null;
    }
    Ok(serialized)
}
pub fn decode(values: &Values) -> graph::Result<Request> {
    let request = Request {
        source: serde_json::from_value(
            values.get("source").ok_or_else(|| fault("source"))?.clone(),
        )
        .map_err(|_| fault("source"))?,
        settings: serde_json::from_value(
            values.get("speech").ok_or_else(|| fault("speech"))?.clone(),
        )
        .map_err(|_| fault("speech"))?,
    };
    let settings = &request.settings;
    let target = &settings.target;
    if request.source.id.is_empty()
        || settings.install_id.is_empty()
        || settings.voice.is_empty()
        || target.revision < 0
        || target.model.trim().is_empty()
        || target.credential.as_ref().is_some_and(String::is_empty)
    {
        return Err(fault("speech"));
    }
    crate::ai::connections::access::base_url(&target.url)
        .map_err(|_| fault("speech.target.url"))?;
    if let Some(resolution) = &target.audio_resolution
        && (resolution.model != target.model
            || resolution.requested_model.trim().is_empty()
            || resolution.provider.trim().is_empty()
            || resolution.language_code.trim().is_empty()
            || resolution.reason.trim().is_empty())
    {
        return Err(fault("speech.target.audio_resolution"));
    }
    audio::validate_speech(target, &request.speech_input()).map_err(|_| fault("speech.source"))?;
    Ok(request)
}
pub fn register_types(registry: &mut Registry) -> graph::Result<()> {
    registry.define_type(
        contract("speech.captured-settings"),
        Shape::Record(BTreeMap::from([
            (
                "target".into(),
                crate::ai::transport::graph_audio_target::shape(),
            ),
            ("installId".into(), Shape::Text),
            ("languageTag".into(), Shape::Text),
            ("language".into(), Shape::Text),
            ("voice".into(), Shape::Text),
        ])),
    )
}
