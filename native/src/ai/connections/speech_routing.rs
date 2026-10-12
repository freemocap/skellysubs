//! Connect configured speech policy to the existing access route. Language
//! preferences select models, never a different server or authentication identity.
use super::access::{self, Capability, ResolvedTarget};
use crate::configuration::{
    LanguageContext,
    speech::{Catalog, Task},
};
use crate::model::Result;
use rusqlite::Connection;

/// Validate captured access without replacing a model selected before capture.
/// The selection may depend on the service inventory; validation does not repeat discovery.
pub(crate) fn validate_access(
    db: &Connection,
    capability: Capability,
    target: &ResolvedTarget,
) -> Result<()> {
    let current = access::resolve(db, capability)?;
    let requested = target
        .audio_resolution
        .as_ref()
        .map_or(target.model.as_str(), |r| r.requested_model.as_str());
    if current.revision != target.revision
        || current.route != target.route
        || current.url != target.url
        || current.credential != target.credential
        || current.model != requested
        || target
            .audio_resolution
            .as_ref()
            .is_some_and(|r| r.model != target.model)
    {
        return Err(crate::model::AppError::new(
            crate::model::ErrorCode::Conflict,
            "Speech access changed before execution.",
        ));
    }
    Ok(())
}

pub fn resolve(
    db: &Connection,
    capability: Capability,
    context: &LanguageContext,
) -> Result<ResolvedTarget> {
    let mut target = access::resolve(db, capability)?;
    let task = match capability {
        Capability::Chat => return Ok(target),
        Capability::Transcription => Task::Transcription,
        Capability::Speech => Task::Speech,
    };
    let resolution = Catalog::bundled().resolve(
        task,
        context
            .external_tags
            .get("language_tag")
            .map(String::as_str)
            .unwrap_or_default(),
        &context.speech_routes,
        &target.model,
        None,
    )?;
    target.model = resolution.model.clone();
    target.audio_resolution = Some(resolution);
    Ok(target)
}

/// Read configured model availability, not provider health or recognition quality.
/// This does not submit learner content or invoke an inference provider.
pub async fn available(target: &ResolvedTarget, key: &str) -> Result<Vec<String>> {
    #[cfg(feature = "workflow-capture")]
    {
        return crate::ai::transport::workflow::unary::protocol(
        serde_json::json!({"operation":"speech-availability","route":target.route,"model":target.model}),
        key, available_live(target,key)).await;
    }
    #[cfg(not(feature = "workflow-capture"))]
    available_live(target, key).await
}
async fn available_live(target: &ResolvedTarget, key: &str) -> Result<Vec<String>> {
    #[cfg(feature = "workflow-capture")]
    crate::ai::transport::workflow::require_live_network()?;
    let base = target
        .url
        .rsplit_once("/audio/")
        .ok_or_else(|| {
            crate::model::AppError::new(
                crate::model::ErrorCode::Validation,
                "Invalid speech service destination.",
            )
        })?
        .0;
    let client = crate::ai::transport::provider::client()?;
    let mut request = client
        .get(format!("{base}/protocol"))
        .timeout(std::time::Duration::from_secs(15));
    if !key.is_empty() {
        request = request.bearer_auth(key);
    }
    let response = request
        .send()
        .await
        .map_err(|e| crate::diagnostics::response::network(&e, "speech_availability"))?;
    let bytes = crate::ai::hosted::body_with_private(response, &[key]).await?;
    let value: serde_json::Value = serde_json::from_slice(&bytes).map_err(|e| {
        crate::diagnostics::response::json_context(
            &e,
            "speech_availability",
            crate::model::AppError::new(
                crate::model::ErrorCode::Validation,
                "Invalid speech availability response.",
            ),
        )
    })?;
    decode_available(&value, &target.model)
}

fn decode_available(value: &serde_json::Value, default: &str) -> Result<Vec<String>> {
    let invalid = || {
        crate::diagnostics::response::invalid(
            "speech_availability",
            "audio.routing",
            "audio version 3 and speech routing version 1 with available model IDs; update the SkellySpeak service",
            value,
        )
    };
    if value["protocol"] != "skellyspeak"
        || value["version"] != 1
        || value["audio"]["version"] != 3
        || value["audio"]["routing"]["version"] != 1
    {
        return Err(invalid());
    }
    let values = value["audio"]["routing"]["available_models"]
        .as_array()
        .filter(|v| v.len() <= 64)
        .ok_or_else(invalid)?;
    let mut models = Vec::new();
    for value in values {
        let id = value
            .as_str()
            .filter(|id| {
                !id.is_empty()
                    && id.len() <= 128
                    && id
                        .bytes()
                        .all(|c| c.is_ascii_alphanumeric() || b"_./-".contains(&c))
            })
            .ok_or_else(invalid)?;
        if models.iter().any(|v| v == id) {
            return Err(invalid());
        }
        models.push(id.to_owned());
    }
    if !Catalog::bundled().models.contains_key(default)
        && value["audio"]["routing"]["accepts_custom_transcription_models"] == true
    {
        models.push(default.into());
    }
    Ok(models)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    #[test]
    fn fresh_workspace_prefers_v4_and_uses_capability_fallback_for_irish() {
        let dir = tempfile::tempdir().unwrap();
        let mut store =
            crate::storage::store::Store::open(&dir.path().join("routing.sqlite3")).unwrap();
        store.prepare_chat().unwrap();
        store.connection.execute("UPDATE ai_config SET route='custom',custom_config=json_set(custom_config,'$.bearerAuth',json('false'))", []).unwrap();
        for (language, expected) in [
            ("english", "eleven_v4_turbo"),
            ("arabic", "eleven_v4_turbo"),
            ("spanish", "eleven_v4_turbo"),
            ("irish", "eleven_v3"),
            ("cantonese", "eleven_v4_turbo"),
        ] {
            let context = store.config.resolve(language, None, "english").unwrap();
            let target = resolve(&store.connection, Capability::Speech, &context).unwrap();
            assert_eq!(target.model, expected);
            assert_eq!(
                target.audio_resolution.as_ref().unwrap().requested_model,
                "eleven_v4_turbo"
            );
            validate_access(&store.connection, Capability::Speech, &target).unwrap();
        }
        // Subsequent explicit learner choices still win over the bundled defaults.
        store.connection.execute("UPDATE ai_config SET audio_settings=json_set(audio_settings,'$.speech.model','eleven_v3')", []).unwrap();
        let context = store.config.resolve("english", None, "english").unwrap();
        assert_eq!(
            resolve(&store.connection, Capability::Speech, &context)
                .unwrap()
                .model,
            "eleven_v3"
        );
    }
    #[test]
    fn captured_language_model_survives_global_model_difference_but_not_access_changes() {
        let dir = tempfile::tempdir().unwrap();
        let mut store =
            crate::storage::store::Store::open(&dir.path().join("routing.sqlite3")).unwrap();
        store.prepare_chat().unwrap();
        store.connection.execute("UPDATE ai_config SET route='custom',custom_config=json_set(custom_config,'$.bearerAuth',json('false'))", []).unwrap();
        let context = store.config.resolve("irish", None, "english").unwrap();
        let target = resolve(&store.connection, Capability::Transcription, &context).unwrap();
        let global = access::resolve(&store.connection, Capability::Transcription).unwrap();
        assert_ne!(target.model, global.model);
        validate_access(&store.connection, Capability::Transcription, &target).unwrap();
        let mut changed = target.clone();
        changed.model = "different".into();
        assert!(validate_access(&store.connection, Capability::Transcription, &changed).is_err());
        store
            .connection
            .execute("UPDATE ai_config SET revision=revision+1", [])
            .unwrap();
        assert!(validate_access(&store.connection, Capability::Transcription, &target).is_err());
    }
    #[test]
    fn inventory_is_explicit_bounded_and_does_not_imply_custom_support() {
        let mut value = json!({"protocol":"skellyspeak","version":1,"audio":{"version":3,"routing":{
            "version":1,"available_models":["scribe_v2"],"accepts_custom_transcription_models":false}}});
        assert_eq!(decode_available(&value, "custom").unwrap(), ["scribe_v2"]);
        value["audio"]["routing"]["accepts_custom_transcription_models"] = json!(true);
        assert_eq!(
            decode_available(&value, "custom").unwrap(),
            ["scribe_v2", "custom"]
        );
        for invalid in [
            json!(null),
            json!(["scribe_v2", "scribe_v2"]),
            json!(["https://private.invalid"]),
            json!([42]),
        ] {
            value["audio"]["routing"]["available_models"] = invalid;
            assert!(decode_available(&value, "whisper-large-v3").is_err());
        }
        assert!(
            decode_available(
                &json!({"protocol":"skellyspeak","version":1}),
                "whisper-large-v3"
            )
            .is_err()
        );
    }
}

#[cfg(test)]
mod integration_tests {
    use super::*;
    use crate::model::ConnectionRoute;
    use std::io::{Read, Write};
    #[tokio::test]
    async fn configured_inventory_selects_before_capture_without_sending_learner_content() {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let url = format!("http://{}/v1", listener.local_addr().unwrap());
        let target = ResolvedTarget {
            audio_resolution: None,
            route: ConnectionRoute::Custom,
            revision: 1,
            url: format!("{url}/audio/transcriptions"),
            model: "whisper-large-v3".into(),
            credential: None,
        };
        let worker = std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            stream
                .set_read_timeout(Some(std::time::Duration::from_secs(5)))
                .unwrap();
            let mut bytes = Vec::new();
            let mut buf = [0u8; 1024];
            while !bytes.windows(4).any(|part| part == b"\r\n\r\n") {
                let size = stream.read(&mut buf).unwrap();
                assert!(size > 0);
                bytes.extend_from_slice(&buf[..size]);
            }
            let request = String::from_utf8(bytes).unwrap();
            assert!(request.starts_with("GET /v1/protocol "));
            assert!(!request.contains("PRIVATE_LEARNER_TEXT"));
            assert!(!request.to_lowercase().contains("authorization:"));
            let body = r#"{"protocol":"skellyspeak","version":1,"audio":{"version":3,"routing":{"version":1,"available_models":["scribe_v2"],"accepts_custom_transcription_models":false}}}"#;
            write!(
                stream,
                "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                body.len()
            )
            .unwrap();
        });
        let inventory = available(&target, "").await.unwrap();
        let choice = Catalog::bundled()
            .resolve(
                Task::Transcription,
                "ga",
                &Default::default(),
                &target.model,
                Some(&inventory),
            )
            .unwrap();
        assert_eq!(choice.model, "scribe_v2");
        worker.join().unwrap();
    }
}
