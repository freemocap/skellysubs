//! Mobile bridges present system share sheets; desktop exports to Downloads.
use tauri::Manager;

#[cfg(target_os = "ios")]
#[path = "sharing_ios.rs"]
mod ios;

#[cfg(target_os = "android")]
struct ShareBridge(tauri::plugin::PluginHandle<tauri::Wry>);

#[cfg(target_os = "android")]
pub fn plugin() -> tauri::plugin::TauriPlugin<tauri::Wry> {
    tauri::plugin::Builder::new("diagnostic-sharing")
        .setup(|app, api| {
            app.manage(ShareBridge(api.register_android_plugin(
                "com.freemocap.skellyspeak",
                "DiagnosticSharePlugin",
            )?));
            Ok(())
        })
        .build()
}

#[tauri::command]
pub async fn share_diagnostic_logs(app: tauri::AppHandle) -> crate::model::Result<()> {
    prepare_execution_diagnostics(&app).await?;
    #[cfg(target_os = "android")]
    {
        let root = super::log_root()?;
        app.state::<ShareBridge>()
            .0
            .run_mobile_plugin_async::<serde_json::Value>(
                "share",
                serde_json::json!({"root": root, "nativeVersion": env!("CARGO_PKG_VERSION")}),
            )
            .await
            .map_err(|error| {
                use tauri::plugin::mobile::PluginInvokeError;
                let stage = match error {
                    PluginInvokeError::InvokeRejected(ref response) => {
                        match response.code.as_deref() {
                            Some(
                                code @ ("busy" | "locate_logs" | "create_archive"
                                | "collect_metadata" | "write_archive" | "create_share_uri"
                                | "open_share_sheet"),
                            ) => code,
                            _ => "unknown_plugin_error",
                        }
                    }
                    PluginInvokeError::UnreachableWebview => "unreachable_webview",
                    PluginInvokeError::Jni(_) => "jni",
                    PluginInvokeError::CannotDeserializeResponse(_) => "decode_response",
                    PluginInvokeError::CannotSerializePayload(_) => "encode_request",
                };
                crate::model::AppError::new(
                    crate::model::ErrorCode::Internal,
                    format!("Diagnostic log sharing failed at {stage}: {}", crate::diagnostics::response::scrub(&error.to_string(), &[])),
                )
                .with_diagnostics(
                    serde_json::json!({"stage": stage, "message": crate::diagnostics::response::scrub(&error.to_string(), &[])}),
                )
            })?;
        Ok(())
    }
    #[cfg(target_os = "ios")]
    {
        ios::share(&app).await
    }
    #[cfg(not(any(target_os = "android", target_os = "ios")))]
    {
        let _ = app;
        Err(crate::model::AppError::new(
            crate::model::ErrorCode::Validation,
            "Native log sharing is available on mobile devices.",
        ))
    }
}

/// Mobile uses system export UI; desktop saves to Downloads.
#[tauri::command]
pub async fn save_diagnostic_logs(app: tauri::AppHandle) -> crate::model::Result<Option<String>> {
    prepare_execution_diagnostics(&app).await?;
    #[cfg(not(target_os = "ios"))]
    let root = super::log_root()?;
    #[cfg(target_os = "android")]
    {
        app.state::<ShareBridge>()
            .0
            .run_mobile_plugin_async::<Option<String>>(
                "save",
                serde_json::json!({"root":root,"nativeVersion":env!("CARGO_PKG_VERSION")}),
            )
            .await
            .map_err(|cause| {
                super::failures::platform(
                    &cause,
                    "save_logs_android",
                    &[],
                    crate::model::AppError::new(
                        crate::model::ErrorCode::Storage,
                        "Could not save diagnostic logs.",
                    ),
                )
            })
    }
    #[cfg(target_os = "ios")]
    {
        ios::share(&app).await?;
        // The system handles the destination; never claim a private path was saved.
        Ok(None)
    }
    #[cfg(not(any(target_os = "android", target_os = "ios")))]
    {
        let directory = app.path().download_dir();
        let directory = directory.map_err(|cause| {
            super::failures::platform(
                &cause,
                "save_logs_directory",
                &[],
                crate::model::AppError::new(
                    crate::model::ErrorCode::Storage,
                    "Could not locate the log export directory.",
                ),
            )
        })?;
        tauri::async_runtime::spawn_blocking(move || {
            super::archive::save(&root, &directory).map(|p| Some(p.to_string_lossy().into_owned()))
        })
        .await
        .map_err(|cause| {
            super::failures::join(
                &cause,
                "save_logs_worker",
                crate::model::AppError::new(
                    crate::model::ErrorCode::Internal,
                    "Log export worker failed.",
                ),
            )
        })?
    }
}

/// Backfill retained failures before every platform builds its archive. A failed
/// snapshot fails export, rather than publishing an apparently complete bundle.
async fn prepare_execution_diagnostics(app: &tauri::AppHandle) -> crate::model::Result<()> {
    let state = app
        .state::<std::sync::Arc<crate::application::Application>>()
        .inner()
        .clone();
    let root = super::log_root()?;
    tauri::async_runtime::spawn_blocking(move || {
        let guard = state
            .store
            .lock()
            .map_err(|_| super::failures::poisoned(crate::application::internal()))?;
        super::execution::save(guard.as_ref().map(|store| &store.connection), &root)
    })
    .await
    .map_err(|cause| {
        super::failures::join(
            &cause,
            "execution_diagnostics_worker",
            crate::application::internal(),
        )
    })?
}
