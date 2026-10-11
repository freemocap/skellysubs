//! Capability resolution is shared by chat, speech and microphone operations. Adapters
//! consume captured targets; they never choose credentials or fall back to a route.
use crate::ai::connections::credentials;
use crate::ai::hosted;
use crate::ai::transport::provider;
use crate::application::Application;
use crate::conversations::execution;
use crate::model::*;
use rusqlite::Connection;
use serde::{Deserialize, Serialize};
use std::{sync::Arc, time::Duration};
use zeroize::Zeroizing;

const DEFAULT_CUSTOM_BASE_URL: &str = "http://127.0.0.1:8765/v1";

#[derive(Clone, Copy)]
pub enum Capability {
    Chat,
    Transcription,
    Speech,
}

/// Recheck the captured destination after asynchronous credential/protocol work.
/// Model choice remains captured; profile changes are handled by owner authority.
pub fn check_captured(
    db: &Connection,
    capability: Capability,
    captured: &ResolvedTarget,
) -> Result<()> {
    let current = resolve(db, capability)?;
    if current.route != captured.route
        || current.url != captured.url
        || current.credential != captured.credential
    {
        return Err(AppError::new(
            ErrorCode::Conflict,
            "AI destination or credentials changed before dispatch. Retry using current settings.",
        ));
    }
    Ok(())
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResolvedTarget {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub audio_resolution: Option<crate::configuration::speech::Resolution>,
    pub route: ConnectionRoute,
    pub revision: i32,
    pub url: String,
    pub model: String,
    pub credential: Option<String>,
}
fn error(message: impl Into<String>) -> AppError {
    AppError::new(ErrorCode::Validation, message)
}
fn conflict() -> AppError {
    AppError::new(
        ErrorCode::Conflict,
        "AI settings changed. Reload before continuing.",
    )
}
pub fn settings(db: &Connection) -> Result<AccessSettings> {
    let (revision, custom, config): (i32, bool, String) = db.query_row(
        "SELECT revision,custom_credential_id IS NOT NULL,custom_config FROM ai_config",
        [],
        |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
    )?;
    Ok(AccessSettings {
        credential_previews: None,
        custom_url_is_unsaved_default: false,
        revision,
        custom_key_configured: custom,
        custom: serde_json::from_str(&config)?,
    })
}
pub fn validate_custom(value: &CustomEndpoint) -> Result<()> {
    base_url(&value.base_url)?;
    Ok(())
}
pub fn base_url(value: &str) -> Result<reqwest::Url> {
    let url = reqwest::Url::parse(value).map_err(|_| {
        error("Enter a complete API base URL, including https:// or http:// for loopback.")
    })?;
    let local = url.host_str().is_some_and(|h| {
        h == "localhost"
            || h.trim_matches(['[', ']'])
                .parse::<std::net::IpAddr>()
                .is_ok_and(|ip| ip.is_loopback())
    });
    if !(url.scheme() == "https" || (url.scheme() == "http" && local))
        || url.host_str().is_none()
        || !url.username().is_empty()
        || url.password().is_some()
        || url.query().is_some()
        || url.fragment().is_some()
    {
        return Err(error(
            "Use HTTPS, or HTTP on localhost/loopback. Put authentication in the key field; URLs cannot contain credentials, query strings or fragments.",
        ));
    }
    Ok(url)
}

fn validate_key_destination(
    saved: &AccessSettings,
    custom: Option<&CustomEndpoint>,
    replacing_or_removing: bool,
) -> Result<()> {
    if let Some(custom) = custom
        && saved.custom_key_configured
        && !replacing_or_removing
        && base_url(&saved.custom.base_url)?
            .as_str()
            .trim_end_matches('/')
            != base_url(&custom.base_url)?.as_str().trim_end_matches('/')
    {
        return Err(error(
            "Enter a replacement key for this API base URL, or restore the saved URL and remove its key first.",
        ));
    }
    Ok(())
}
pub fn resolve(db: &Connection, capability: Capability) -> Result<ResolvedTarget> {
    let config = crate::ai::connections::configuration::config(db)?;
    let access = settings(db)?;
    let route = config.route;
    let (base, model, column) = match (route, capability) {
        (ConnectionRoute::Hosted, Capability::Chat) => (
            format!("{}/v1", hosted::ORIGIN),
            config.standard_model,
            "hosted_credential_id",
        ),
        (ConnectionRoute::Hosted, Capability::Speech) => (
            format!("{}/v1", hosted::ORIGIN),
            config.audio.speech.model.clone(),
            "hosted_credential_id",
        ),
        (ConnectionRoute::Hosted, Capability::Transcription) => (
            format!("{}/v1", hosted::ORIGIN),
            config.audio.transcription.model.clone(),
            "hosted_credential_id",
        ),
        (ConnectionRoute::Custom, _) => {
            validate_custom(&access.custom)?;
            let model = match capability {
                Capability::Chat => config.standard_model.clone(),
                Capability::Speech => config.audio.speech.model.clone(),
                Capability::Transcription => config.audio.transcription.model.clone(),
            };
            (
                access.custom.base_url.clone(),
                model,
                "custom_credential_id",
            )
        }
    };
    let needs_key = route != ConnectionRoute::Custom || access.custom.bearer_auth;
    let credential = if needs_key {
        let id: Option<String> =
            db.query_row(&format!("SELECT {column} FROM ai_config"), [], |r| r.get(0))?;
        Some(id.ok_or_else(|| {
            error(match (route, capability) {
                (ConnectionRoute::Hosted, _) => "Sign in with Google in AI access settings.",
                (ConnectionRoute::Custom, _) => {
                    "Add the custom server session token in AI access settings."
                }
            })
        })?)
    } else {
        None
    };
    let path = match capability {
        Capability::Chat => "operations",
        Capability::Speech => "audio/speech",
        Capability::Transcription => "audio/transcriptions",
    };
    Ok(ResolvedTarget {
        audio_resolution: None,
        route,
        revision: config.revision,
        url: format!("{}/{path}", base.trim_end_matches('/')),
        model,
        credential,
    })
}
#[tauri::command]
pub async fn get_access_settings(
    state: tauri::State<'_, Arc<Application>>,
) -> Result<AccessSettings> {
    let (mut settings, ids) = {
        let store = state.lock()?;
        let settings = settings_for_editing(&store.connection)?;
        let ids: [Option<String>; 1] = store.connection.query_row(
            "SELECT custom_credential_id FROM ai_config",
            [],
            |row| Ok([row.get(0)?]),
        )?;
        (settings, ids)
    };
    let mut previews = std::collections::BTreeMap::new();
    for (provider, id) in ["custom"].into_iter().zip(ids) {
        if let Some(id) = id {
            let secret = crate::application::read_secret(id).await?;
            previews.insert(provider.into(), credentials::preview(&secret));
        }
    }
    if self::settings(&state.lock()?.connection)?.revision != settings.revision {
        return Err(conflict());
    }
    settings.credential_previews = Some(previews);
    Ok(settings)
}

fn settings_for_editing(db: &Connection) -> Result<AccessSettings> {
    let mut value = settings(db)?;
    // This is an editable default, never a resolver fallback or storage rewrite.
    if value.custom.base_url.is_empty() && !value.custom_key_configured {
        value.custom.base_url = DEFAULT_CUSTOM_BASE_URL.into();
        value.custom_url_is_unsaved_default = true;
    }
    Ok(value)
}

// Saving a custom server must not overwrite or activate hosted access.
#[tauri::command]
pub async fn save_access_settings(
    state: tauri::State<'_, Arc<Application>>,
    expected_revision: i32,
    custom: Option<CustomEndpoint>,
    session_token: Option<String>,
    remove_token: bool,
) -> Result<AccessSettings> {
    if custom.is_none() {
        return Err(error("Custom server settings are required."));
    }
    let key = session_token.map(|s| Zeroizing::new(s.trim().to_owned()));
    validate_save_input(
        custom.as_ref(),
        key.as_ref().map(|k| k.as_str()),
        remove_token,
    )
    .map_err(|(code, error)| {
        crate::diagnostics::native_event(code, &[]);
        *error
    })?;
    let state = state.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        let prepare = |store: &mut crate::storage::store::Store| -> Result<()> {
            if settings(&store.connection)?.revision != expected_revision {
                crate::diagnostics::native_event("access_save_revision_conflict", &[]);
                return Err(conflict());
            }
            validate_key_destination(&settings(&store.connection)?, custom.as_ref(), key.is_some() || remove_token)
                .inspect_err(|_| { crate::diagnostics::native_event("access_save_destination_changed", &[]); })
        };
        let commit = |store: &mut crate::storage::store::Store, id: Option<&str>| -> Result<AccessSettings> {
            let tx = store.connection.transaction()?;
            if settings(&tx)?.revision != expected_revision {
                crate::diagnostics::native_event("access_save_revision_conflict", &[]);
                return Err(conflict());
            }
            let column = "custom_credential_id";
            if id.is_some() || remove_token {
                tx.execute(&format!("INSERT OR IGNORE INTO credential_cleanup SELECT {column} FROM ai_config WHERE {column} IS NOT NULL"),[])?;
                tx.execute(&format!("UPDATE ai_config SET {column}=?1"),[id])?;
                if let Some(id) = id { tx.execute("DELETE FROM credential_cleanup WHERE id=?1",[id])?; }
            }
            if let Some(custom) = &custom {
                tx.execute("UPDATE ai_config SET custom_config=?1",[serde_json::to_string(custom)?])?;
            }
            tx.execute("UPDATE ai_config SET revision=revision+1",[])?;
            execution::invalidate(&tx,Some(ConnectionRoute::Custom))?;
            tx.execute("UPDATE metadata SET revision=revision+1",[])?;
            tx.commit()?;
            settings(&store.connection)
        };
        if let Some(key) = &key {
            state.write_credential_with(prepare, |id| credentials::save(id,key),
                |store,id| commit(store,Some(id)), credentials::remove)
        } else {
            let result = {
                let mut store = state.lock()?;
                prepare(&mut store)?;
                commit(&mut store,None)
            };
            state.clean_credentials()?;
            result
        }
    }).await.map_err(|cause| crate::diagnostics::failures::join(&cause, "access.rs", error("Saving AI access settings stopped unexpectedly.")))?
}

fn validate_save_input(
    custom: Option<&CustomEndpoint>,
    key: Option<&str>,
    remove_token: bool,
) -> std::result::Result<(), (&'static str, Box<AppError>)> {
    if let Some(value) = custom {
        base_url(&value.base_url).map_err(|e| ("access_save_url_invalid", Box::new(e)))?;
        validate_custom(value).map_err(|e| ("access_save_endpoint_invalid", Box::new(e)))?;
    }
    if let Some(key) = key {
        credentials::validate_session_token(key)
            .map_err(|e| ("access_save_key_invalid", Box::new(e)))?;
    }
    if remove_token && key.is_some() {
        return Err((
            "access_save_remove_replace_conflict",
            Box::new(error("Remove or replace the session token, not both.")),
        ));
    }
    Ok(())
}

pub async fn response_bytes(
    mut response: reqwest::Response,
    label: &str,
    _route: ConnectionRoute,
    limit: usize,
) -> Result<Vec<u8>> {
    if !response.status().is_success() {
        return Err(crate::diagnostics::response::http_error(response, label, &[]).await);
    }
    let mut body = Vec::new();
    while let Some(chunk) = response.chunk().await.map_err(|cause| {
        crate::diagnostics::response::network_context(
            &cause,
            "access_response_body",
            error(format!("{label}: response interrupted.")),
        )
    })? {
        crate::ai::content::capture::chunk(&chunk)?;
        if body.len() + chunk.len() > limit {
            return Err(error(format!("{label}: response exceeds the size limit.")));
        }
        body.extend_from_slice(&chunk);
    }
    crate::ai::content::capture::complete()?;
    Ok(body)
}
#[tauri::command]
pub async fn check_access(
    state: tauri::State<'_, Arc<Application>>,
    expected_revision: i32,
) -> Result<AccessCheck> {
    #[cfg(feature = "workflow-capture")]
    {
        return crate::ai::transport::workflow::unary::protocol(
            serde_json::json!({"operation":"custom-access-check","revision":expected_revision}),
            "",
            check_access_live(state, expected_revision),
        )
        .await;
    }
    #[cfg(not(feature = "workflow-capture"))]
    check_access_live(state, expected_revision).await
}
async fn check_access_live(
    state: tauri::State<'_, Arc<Application>>,
    expected_revision: i32,
) -> Result<AccessCheck> {
    #[cfg(feature = "workflow-capture")]
    crate::ai::transport::workflow::require_live_network()?;
    let (url, credential) = {
        let store = state.lock()?;
        let access = settings(&store.connection)?;
        if access.revision != expected_revision {
            return Err(conflict());
        }
        validate_custom(&access.custom)?;
        let column = "custom_credential_id";
        let credential: Option<String> =
            store
                .connection
                .query_row(&format!("SELECT {column} FROM ai_config"), [], |r| r.get(0))?;
        let key = if !access.custom.bearer_auth {
            None
        } else {
            Some(credential.ok_or_else(|| error("Save a server session token first."))?)
        };
        (
            format!(
                "{}/protocol?verify_providers=true",
                access.custom.base_url.trim_end_matches('/')
            ),
            key,
        )
    };
    let key = match credential {
        Some(id) => crate::application::read_secret(id).await?,
        None => Zeroizing::new(String::new()),
    };
    if state.lock()?.connection_config()?.revision != expected_revision {
        return Err(conflict());
    }
    let request = provider::client()?
        .get(url)
        .timeout(Duration::from_secs(15));
    let request = if key.is_empty() {
        request
    } else {
        request.bearer_auth(key.as_str())
    };
    let response = request.send().await.map_err(|cause| {
        crate::diagnostics::response::network_context(
            &cause,
            "connection_check",
            error("Connection check failed. Check the endpoint and network."),
        )
    })?;
    if !response.status().is_success() {
        return Err(crate::diagnostics::response::http_error(
            response,
            "Connection check",
            &[key.as_str()],
        )
        .await);
    }
    let value: serde_json::Value = serde_json::from_slice(
        &response_bytes(
            response,
            "Connection check",
            ConnectionRoute::Custom,
            262144,
        )
        .await?,
    )
    .map_err(|cause| {
        crate::diagnostics::response::json_context(
            &cause,
            "connection_check_json",
            error("Endpoint returned invalid JSON."),
        )
    })?;
    {
        if value["protocol"] != "skellyspeak"
            || value["version"].as_u64() != Some(1)
            || value["max_items"].as_u64() != Some(8)
            || !value["operations_versions"]
                .as_array()
                .is_some_and(|v| v.iter().any(|v| v == 3))
            || value["audio"]["version"] != 3
        {
            return Err(error(
                "This app requires the version 3 inference/audio service. Update the app and server together.",
            ));
        }
        let store = state.lock()?;
        if store.connection_config()?.revision != expected_revision {
            return Err(conflict());
        }
        // Capability model lists are recommendations, not availability gates.
        // The selected provider validates the configured model during inference.
        let providers: Vec<ProviderCredentialCheck> =
            serde_json::from_value(value["providers"].clone()).map_err(|cause| {
                crate::diagnostics::response::json_context(
                    &cause,
                    "access.rs_decode",
                    error(
                        "Update the custom server to support internal provider credential checks.",
                    ),
                )
            })?;
        if !(2..=3).contains(&providers.len())
            || providers
                .iter()
                .filter(|p| p.provider == "OPENROUTER")
                .count()
                != 1
            || providers.iter().any(|p| {
                !["OPENROUTER", "GROQ", "ELEVENLABS"].contains(&p.provider.as_str())
                    || providers
                        .iter()
                        .filter(|other| other.provider == p.provider)
                        .count()
                        != 1
            })
            || providers.iter().any(|p| {
                !matches!(
                    p.state.as_str(),
                    "accepted" | "rejected" | "unreachable" | "invalid_response"
                )
            })
        {
            return Err(error("Server returned invalid provider credential checks."));
        }
        Ok(AccessCheck { providers })
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn save_validation_reports_allowlisted_reason_without_private_input() {
        let mut custom = crate::model::CustomEndpoint {
            base_url: String::new(),
            bearer_auth: true,
        };
        assert_eq!(
            super::validate_save_input(Some(&custom), None, false)
                .unwrap_err()
                .0,
            "access_save_url_invalid"
        );
        custom.base_url = "http://127.0.0.1:8765/v1".into();
        assert!(super::validate_save_input(Some(&custom), Some("fixture-secret"), false).is_ok());
        let (code, error) =
            super::validate_save_input(Some(&custom), Some("PRIVATE KEY"), false).unwrap_err();
        assert_eq!(code, "access_save_key_invalid");
        assert!(!error.message.contains("PRIVATE"));
        assert_eq!(
            super::validate_save_input(None, Some("fixture-secret"), true)
                .unwrap_err()
                .0,
            "access_save_remove_replace_conflict"
        );
    }

    use super::*;
    fn db() -> Connection {
        let db = Connection::open_in_memory().unwrap();
        db.execute_batch(include_str!("../../storage/schemas/schema.sql"))
            .unwrap();
        db
    }
    #[test]
    fn fresh_custom_setup_uses_shared_models_and_voice_enabled() {
        let database = db();
        let endpoint = settings(&database).unwrap().custom;
        let models = crate::ai::connections::configuration::config(&database).unwrap();
        assert_eq!(models.standard_model, "google/gemini-2.5-flash");
        assert_eq!(models.fast_model, "google/gemini-2.5-flash-lite");
        assert_eq!(models.audio.transcription.model, "whisper-large-v3");
        assert!(endpoint.bearer_auth);
        assert_eq!(endpoint.base_url, DEFAULT_CUSTOM_BASE_URL);
        validate_custom(&endpoint).unwrap();
    }
    #[test]
    fn editable_local_url_default_never_rewrites_or_resolves_unsaved_settings() {
        let database = db();
        for saved_url in ["", "https://fixture.example/v1"] {
            database
                .execute(
                    "UPDATE ai_config SET custom_config=json_set(custom_config,'$.baseUrl',?1)",
                    [saved_url],
                )
                .unwrap();
            let before: (i32, String, String) = database
                .query_row(
                    "SELECT revision,route,custom_config FROM ai_config",
                    [],
                    |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
                )
                .unwrap();
            let edited = settings_for_editing(&database).unwrap();
            assert_eq!(
                edited.custom.base_url,
                if saved_url.is_empty() {
                    DEFAULT_CUSTOM_BASE_URL
                } else {
                    saved_url
                }
            );
            assert_eq!(settings(&database).unwrap().custom.base_url, saved_url);
            assert!(edited.custom.bearer_auth);
            let after: (i32, String, String) = database
                .query_row(
                    "SELECT revision,route,custom_config FROM ai_config",
                    [],
                    |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
                )
                .unwrap();
            assert_eq!(after, before);
        }
        database.execute("UPDATE ai_config SET route='custom',custom_config=json_set(custom_config,'$.baseUrl','')", []).unwrap();
        assert!(resolve(&database, Capability::Chat).is_err());
        database.execute("UPDATE ai_config SET custom_config=json_set(custom_config,'$.bearerAuth',json('false'))", []).unwrap();
        let mut draft = settings_for_editing(&database).unwrap().custom;
        assert!(!draft.bearer_auth);
        assert_eq!(draft.base_url, DEFAULT_CUSTOM_BASE_URL);
        draft.base_url.clear();
        assert_eq!(
            validate_save_input(Some(&draft), None, false)
                .unwrap_err()
                .0,
            "access_save_url_invalid"
        );
        database
            .execute(
                "UPDATE ai_config SET custom_credential_id='fixture-existing-id'",
                [],
            )
            .unwrap();
        assert!(
            settings_for_editing(&database)
                .unwrap()
                .custom
                .base_url
                .is_empty()
        );
    }
    fn custom(db: &Connection, auth: bool) {
        let value = CustomEndpoint {
            base_url: "http://127.0.0.1:1234/v1/".into(),
            bearer_auth: auth,
        };
        db.execute(
            "UPDATE ai_config SET route='custom',custom_config=?1",
            [serde_json::to_string(&value).unwrap()],
        )
        .unwrap();
    }
    #[test]
    fn custom_transport_policy_rejects_credential_urls_and_remote_cleartext() {
        for url in [
            "http://localhost:1234/v1",
            "http://127.0.0.1:9000",
            "http://[::1]:8000/v1",
            "https://example.com/api/v1",
        ] {
            base_url(url).unwrap();
        }
        for url in [
            "http://192.168.1.2/v1",
            "http://localhost.evil.test/v1",
            "https://key:secret@example.com",
            "https://example.com?key=secret",
            "https://example.com#secret",
            "file:///tmp/server",
            "ftp://localhost/v1",
        ] {
            assert!(base_url(url).is_err(), "{url}");
        }
    }
    #[test]
    fn saved_custom_key_cannot_follow_a_changed_destination() {
        let db = db();
        custom(&db, true);
        db.execute("UPDATE ai_config SET custom_credential_id='saved-key'", [])
            .unwrap();
        let saved = settings(&db).unwrap();
        let mut next = saved.custom.clone();
        validate_key_destination(&saved, Some(&next), false).unwrap();
        for destination in [
            "https://other.example/v1",
            "http://127.0.0.1:4321/v1",
            "http://127.0.0.1:1234/other-tenant",
        ] {
            next.base_url = destination.into();
            assert!(validate_key_destination(&saved, Some(&next), false).is_err());
            validate_key_destination(&saved, Some(&next), true).unwrap();
        }
        validate_key_destination(&saved, None, false).unwrap();
    }

    #[test]
    fn custom_no_auth_uses_shared_chat_and_transcription_models() {
        let db = db();
        custom(&db, false);
        db.execute(
            "UPDATE ai_config SET hosted_credential_id='other-key',custom_credential_id='unused-secret'",
            [],
        )
        .unwrap();
        let chat = resolve(&db, Capability::Chat).unwrap();
        assert!(chat.credential.is_none());
        assert_eq!(chat.url, "http://127.0.0.1:1234/v1/operations");
        assert_eq!(chat.model, "google/gemini-2.5-flash");
        assert_eq!(
            resolve(&db, Capability::Transcription).unwrap().model,
            "whisper-large-v3"
        );
        custom(&db, true);
        let audio = resolve(&db, Capability::Transcription).unwrap();
        assert_eq!(audio.model, "whisper-large-v3");
        assert_eq!(audio.credential.as_deref(), Some("unused-secret"));
        db.execute("UPDATE ai_config SET custom_credential_id=NULL", [])
            .unwrap();
        assert!(
            !crate::ai::connections::configuration::config(&db)
                .unwrap()
                .configured
        );
        assert!(resolve(&db, Capability::Chat).is_err());
    }
    #[test]
    fn service_routes_own_every_capability_and_reject_the_removed_route() {
        let db = db();
        db.execute(
            "UPDATE ai_config SET hosted_credential_id='hosted',custom_credential_id='custom'",
            [],
        )
        .unwrap();
        for route in ["hosted", "custom"] {
            db.execute("UPDATE ai_config SET route=?1", [route])
                .unwrap();
            for (capability, endpoint) in [
                (Capability::Chat, "operations"),
                (Capability::Speech, "audio/speech"),
                (Capability::Transcription, "audio/transcriptions"),
            ] {
                let target = resolve(&db, capability).unwrap();
                assert!(target.url.ends_with(endpoint));
                assert_eq!(target.credential.as_deref(), Some(route));
            }
        }
        assert!(serde_json::from_str::<ConnectionRoute>("\"openrouter\"").is_err());
        assert!(
            db.execute("UPDATE ai_config SET route='openrouter'", [])
                .is_err()
        );
    }
}
