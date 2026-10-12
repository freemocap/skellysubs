//! Development convenience only. Never returns the credential to the webview.
use super::*;
use crate::model::AccessSettings;

#[tauri::command]
pub(in crate::application) fn local_server_available() -> bool {
    cfg!(all(debug_assertions, desktop))
}

#[tauri::command]
pub(in crate::application) async fn connect_local_server(
    state: tauri::State<'_, Arc<Application>>,
    expected_revision: i32,
) -> Result<AccessSettings> {
    #[cfg(feature = "workflow-capture")]
    crate::ai::transport::workflow::require_live_network()?;
    #[cfg(all(debug_assertions, desktop))]
    {
        let token = read_token(
            &std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("../server/.local-server/session-token.txt"),
        )?;
        access::save_access_settings(
            state,
            expected_revision,
            Some(crate::model::CustomEndpoint {
                base_url: "http://127.0.0.1:8765/v1".into(),
                bearer_auth: true,
            }),
            Some(token.to_string()),
            false,
        )
        .await
    }
    #[cfg(not(all(debug_assertions, desktop)))]
    {
        let _ = (state, expected_revision);
        Err(AppError::new(
            ErrorCode::Validation,
            "Local server setup is available only in desktop development builds.",
        ))
    }
}

#[tauri::command]
pub(in crate::application) async fn open_local_admin(app: tauri::AppHandle) -> Result<()> {
    #[cfg(feature = "workflow-capture")]
    crate::ai::transport::workflow::require_live_network()?;
    #[cfg(all(debug_assertions, desktop))]
    {
        use tauri_plugin_opener::OpenerExt;
        let failure = || {
            AppError::new(
                ErrorCode::Credential,
                "Cannot open local administration. Start the local server from this checkout, then try again.",
            )
        };
        let token = read_token(
            &std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("../server/.local-server/admin-token.txt"),
        )?;
        let client = reqwest::Client::builder()
            .no_proxy()
            .redirect(reqwest::redirect::Policy::none())
            .timeout(std::time::Duration::from_secs(5))
            .build()
            .map_err(|cause| {
                crate::diagnostics::response::network_context(&cause, "local_admin_http", failure())
            })?;
        let mut response = client
            .post("http://127.0.0.1:8765/admin/local/ticket")
            .bearer_auth(token.as_str())
            .send()
            .await
            .map_err(|cause| {
                crate::diagnostics::response::network_context(&cause, "local_admin_http", failure())
            })?
            .error_for_status()
            .map_err(|cause| {
                crate::diagnostics::response::network_context(&cause, "local_admin_http", failure())
            })?;
        let mut bytes = Vec::new();
        while let Some(chunk) = response.chunk().await.map_err(|cause| {
            crate::diagnostics::response::network_context(&cause, "local_admin_http", failure())
        })? {
            if bytes.len() + chunk.len() > 1024 {
                return Err(failure());
            }
            bytes.extend_from_slice(&chunk);
        }
        let value: serde_json::Value = serde_json::from_slice(&bytes).map_err(|cause| {
            crate::diagnostics::response::json_context(&cause, "local_admin_json", failure())
        })?;
        let ticket = value
            .get("ticket")
            .and_then(|v| v.as_str())
            .ok_or_else(failure)?;
        if ticket.len() != 43
            || !ticket
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
        {
            return Err(failure());
        }
        app.opener()
            .open_url(
                format!("http://127.0.0.1:8765/admin/local/login#{ticket}"),
                None::<&str>,
            )
            .map_err(|_| failure())?;
        Ok(())
    }
    #[cfg(not(all(debug_assertions, desktop)))]
    {
        let _ = app;
        Err(AppError::new(
            ErrorCode::Validation,
            "Local administration is available only in desktop development builds.",
        ))
    }
}

#[cfg(all(debug_assertions, desktop))]
fn read_token(path: &std::path::Path) -> Result<Zeroizing<String>> {
    use std::io::Read;
    let invalid = || {
        AppError::new(
            ErrorCode::Credential,
            "Cannot read the local server token. Start the local server from this checkout, then try again.",
        )
    };
    // Reject symlinks, special files and oversized input. No arbitrary path is
    // accepted through IPC, and session.json (the signing key) is never opened.
    for ancestor in path.ancestors() {
        if std::fs::symlink_metadata(ancestor)
            .map_err(|cause| {
                crate::diagnostics::response::io_context(&cause, "local_token_read", invalid())
            })?
            .is_symlink()
        {
            return Err(invalid());
        }
    }
    let metadata = std::fs::metadata(path).map_err(|cause| {
        crate::diagnostics::response::io_context(&cause, "local_token_read", invalid())
    })?;
    if !metadata.is_file() || metadata.len() > 4096 {
        return Err(invalid());
    }
    let mut token = Zeroizing::new(String::new());
    std::fs::File::open(path)
        .map_err(|cause| {
            crate::diagnostics::response::io_context(&cause, "local_token_read", invalid())
        })?
        .take(4097)
        .read_to_string(&mut token)
        .map_err(|cause| {
            crate::diagnostics::response::io_context(&cause, "local_token_read", invalid())
        })?;
    let trimmed = token.trim();
    if trimmed.len() > 4096
        || trimmed.split('.').count() != 3
        || trimmed.split('.').any(|part| {
            part.is_empty()
                || !part
                    .bytes()
                    .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
        })
    {
        return Err(invalid());
    }
    Ok(Zeroizing::new(trimmed.to_owned()))
}

#[cfg(all(test, debug_assertions, desktop))]
mod tests {
    use super::*;

    #[test]
    fn reads_only_token_and_rejects_missing_malformed_and_large_files() {
        let directory = tempfile::tempdir().unwrap();
        let directory = directory.path().canonicalize().unwrap();
        let path = directory.join("session-token.txt");
        assert!(read_token(&path).is_err());
        std::fs::write(&path, "header.payload.signature\n").unwrap();
        assert_eq!(
            read_token(&path).unwrap().as_str(),
            "header.payload.signature"
        );
        for value in [
            "",
            "secret",
            "a.b.c.d",
            "a..c",
            "a.b.secret with spaces",
            &"a".repeat(4097),
        ] {
            std::fs::write(&path, value).unwrap();
            let error = read_token(&path).unwrap_err();
            assert!(!error.message.contains("secret"));
        }
    }

    #[cfg(unix)]
    #[test]
    fn rejects_symlink_token() {
        let directory = tempfile::tempdir().unwrap();
        let directory = directory.path().canonicalize().unwrap();
        let target = directory.join("private");
        std::fs::write(&target, "a.b.c").unwrap();
        let path = directory.join("session-token.txt");
        std::os::unix::fs::symlink(target, &path).unwrap();
        assert!(read_token(&path).is_err());
    }
}
