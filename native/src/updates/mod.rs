//! Fixed-origin release discovery. Desktop payload signatures are verified by Tauri.
use crate::model::AppError;
use crate::model::ErrorCode;
use crate::model::Result;
use serde::Serialize;
use std::time::Duration;

#[tauri::command]
pub fn get_update_channel() -> &'static str {
    if cfg!(debug_assertions) {
        "development"
    } else if cfg!(target_os = "android") {
        "download"
    } else if cfg!(target_os = "ios") {
        "app-store"
    } else {
        "install"
    }
}
#[derive(Serialize)]
pub struct LatestRelease {
    version: String,
    url: String,
    notes: String,
}
fn failure(message: &str) -> AppError {
    AppError::new(ErrorCode::Provider, message)
}
#[tauri::command]
pub async fn latest_github_release() -> Result<LatestRelease> {
    #[cfg(feature = "workflow-capture")]
    crate::ai::transport::workflow::require_live_network()?;
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(20))
        .redirect(reqwest::redirect::Policy::none())
        .user_agent("SkellySpeak-update-check")
        .build()
        .map_err(|cause| {
            crate::diagnostics::response::network_context(
                &cause,
                "release_http_client",
                failure("Could not initialize release discovery."),
            )
        })?;
    let mut response = client
        .get("https://api.github.com/repos/freemocap/skellyspeak/releases/latest")
        .send()
        .await
        .map_err(|cause| {
            crate::diagnostics::response::network_context(
                &cause,
                "release_discovery",
                failure("Could not reach GitHub releases."),
            )
        })?;
    if !response.status().is_success() {
        return Err(crate::diagnostics::response::http_error(
            response,
            "GitHub release discovery",
            &[],
        )
        .await);
    }
    let mut bytes = Vec::new();
    while let Some(chunk) = response.chunk().await.map_err(|cause| {
        crate::diagnostics::response::network_context(
            &cause,
            "release_response_body",
            failure("Could not read the release response."),
        )
    })? {
        if bytes.len() + chunk.len() > 512 * 1024 {
            return Err(failure("Release response exceeds the size limit."));
        }
        bytes.extend_from_slice(&chunk);
    }
    let value: serde_json::Value = serde_json::from_slice(&bytes).map_err(|cause| {
        crate::diagnostics::response::json_context(
            &cause,
            "release_response_json",
            failure("Invalid release response."),
        )
    })?;
    let tag = value["tag_name"]
        .as_str()
        .ok_or_else(|| failure("Release version is missing."))?;
    let version = tag
        .strip_prefix('v')
        .ok_or_else(|| failure("Invalid release tag."))?;
    if !regex::Regex::new(r"^\d+\.\d+\.\d+$")
        .unwrap()
        .is_match(version)
    {
        return Err(failure("Invalid stable release version."));
    }
    if value["draft"] != false || value["prerelease"] != false {
        return Err(failure("The update is not a published stable release."));
    }
    Ok(LatestRelease {
        version: version.into(),
        url: format!("https://github.com/freemocap/skellyspeak/releases/tag/{tag}"),
        notes: value["body"].as_str().unwrap_or("").into(),
    })
}
