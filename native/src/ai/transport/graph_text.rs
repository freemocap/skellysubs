//! Secret-free, typed transport settings shared by executable text operations.
//! This module supplies no topology, scheduling, authority or result publication.
use crate::ai::{
    connections::access::ResolvedTarget,
    graph::{self, *},
};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[cfg(test)]
mod tests;

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct Target {
    route: crate::model::ConnectionRoute,
    revision: i32,
    url: String,
    model: String,
}

#[derive(Clone)]
pub(crate) struct Settings {
    pub target: ResolvedTarget,
    pub install_id: String,
    pub temperature: f64,
}

fn fault(path: &str) -> Fault {
    Fault {
        code: "text_transport_input_invalid".into(),
        path: path.into(),
    }
}
fn contract(name: &str) -> Contract {
    Contract::new(name, 1)
}

pub(crate) fn inputs() -> Ports {
    BTreeMap::from([
        (
            "target".into(),
            Port {
                contract: contract("ai.text-target"),
                optional: false,
            },
        ),
        (
            "credential".into(),
            Port {
                contract: contract("ai.credential-reference"),
                optional: true,
            },
        ),
        (
            "install_id".into(),
            Port {
                contract: contract("ai.install-id"),
                optional: false,
            },
        ),
        (
            "temperature".into(),
            Port {
                contract: contract("ai.temperature"),
                optional: false,
            },
        ),
    ])
}

/// Temperature contract v1 retains its canonical decimal encoding so existing
/// coach artifact identities and persisted captures do not change. The newer
/// Number shape does not implicitly migrate existing semantic contracts.
fn temperature(value: &str) -> graph::Result<f64> {
    let parsed: f64 = value.parse().map_err(|_| fault("temperature"))?;
    crate::ai::transport::provider::validate_temperature(parsed)
        .map_err(|_| fault("temperature"))?;
    if parsed.to_string() != value {
        return Err(fault("temperature"));
    }
    Ok(parsed)
}

pub(crate) fn decode(values: &Values) -> graph::Result<Settings> {
    let target: Target =
        serde_json::from_value(values.get("target").ok_or_else(|| fault("target"))?.clone())
            .map_err(|_| fault("target"))?;
    if target.revision < 0 || target.model.trim().is_empty() {
        return Err(fault("target"));
    }
    crate::ai::connections::access::base_url(&target.url).map_err(|_| fault("target.url"))?;
    let credential = values
        .get("credential")
        .map(|v| v.as_str().ok_or_else(|| fault("credential")))
        .transpose()?;
    if credential.is_some_and(str::is_empty) {
        return Err(fault("credential"));
    }
    let install_id = values
        .get("install_id")
        .ok_or_else(|| fault("install_id"))?
        .as_str()
        .ok_or_else(|| fault("install_id"))?;
    if install_id.is_empty() {
        return Err(fault("install_id"));
    }
    Ok(Settings {
        target: ResolvedTarget {
            audio_resolution: None,
            route: target.route,
            revision: target.revision,
            url: target.url,
            model: target.model,
            credential: credential.map(str::to_owned),
        },
        install_id: install_id.into(),
        temperature: temperature(
            values
                .get("temperature")
                .ok_or_else(|| fault("temperature"))?
                .as_str()
                .ok_or_else(|| fault("temperature"))?,
        )?,
    })
}

/// Capture secret-free inputs once at admission. Credential values are references
/// only. Reject audio targets instead of silently dropping their resolution data.
pub(crate) fn capture(
    target: &ResolvedTarget,
    install_id: &str,
    captured_temperature: f64,
) -> graph::Result<Values> {
    if target.audio_resolution.is_some() {
        return Err(fault("target.audio_resolution"));
    }
    let target_value = Target {
        route: target.route,
        revision: target.revision,
        url: target.url.clone(),
        model: target.model.clone(),
    };
    let mut values = BTreeMap::from([
        (
            "target".into(),
            serde_json::to_value(target_value).map_err(|_| fault("target"))?,
        ),
        ("install_id".into(), serde_json::json!(install_id)),
        (
            "temperature".into(),
            serde_json::json!(captured_temperature.to_string()),
        ),
    ]);
    if let Some(credential) = &target.credential {
        values.insert("credential".into(), serde_json::json!(credential));
    }
    decode(&values)?;
    Ok(values)
}

pub(crate) fn register_types(registry: &mut Registry) -> graph::Result<()> {
    registry.define_type(
        contract("ai.text-target"),
        Shape::Record(BTreeMap::from([
            ("route".into(), Shape::Text),
            ("revision".into(), Shape::Integer),
            ("url".into(), Shape::Text),
            ("model".into(), Shape::Text),
        ])),
    )?;
    for name in ["ai.credential-reference", "ai.install-id", "ai.temperature"] {
        registry.define_type(contract(name), Shape::Text)?;
    }
    Ok(())
}
