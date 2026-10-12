use super::*;
use serde_json::json;

fn values() -> Values {
    capture(
        &ResolvedTarget {
            route: crate::model::ConnectionRoute::Custom,
            revision: 9,
            url: "http://127.0.0.1:12345/v1".into(),
            model: "captured-model".into(),
            credential: Some("credential-reference".into()),
            audio_resolution: None,
        },
        "installation",
        0.7,
    )
    .unwrap()
}

#[test]
fn absent_and_invalid_settings_fail_before_transport_without_panicking() {
    let valid = values();
    for field in ["target", "install_id", "temperature"] {
        let mut missing = valid.clone();
        missing.remove(field);
        assert_eq!(decode(&missing).err().unwrap().path, field);
    }
    for (field, value, path) in [
        ("target", json!({}), "target"),
        ("install_id", json!(""), "install_id"),
        ("credential", json!(""), "credential"),
        ("credential", json!(4), "credential"),
        ("temperature", json!("0.70"), "temperature"),
        ("temperature", json!("NaN"), "temperature"),
        ("temperature", json!("3"), "temperature"),
    ] {
        let mut invalid = valid.clone();
        invalid.insert(field.into(), value);
        assert_eq!(decode(&invalid).err().unwrap().path, path);
    }
}

#[test]
fn transport_settings_preserve_captured_authority_and_optional_credentials() {
    let mut values = values();
    let settings = decode(&values).unwrap();
    assert_eq!(settings.target.revision, 9);
    assert_eq!(settings.target.model, "captured-model");
    assert_eq!(
        settings.target.credential.as_deref(),
        Some("credential-reference")
    );
    assert_eq!(settings.install_id, "installation");
    assert_eq!(settings.temperature, 0.7);
    values.remove("credential");
    assert!(decode(&values).unwrap().target.credential.is_none());
    values.get_mut("target").unwrap()["url"] = json!("https://user:secret@example.com");
    let fault = decode(&values).err().unwrap();
    assert_eq!(fault.path, "target.url");
    assert!(!serde_json::to_string(&fault).unwrap().contains("secret"));
}
