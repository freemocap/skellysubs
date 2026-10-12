//! Explicit development-session controls. Ordinary builds reject capture commands.
#[cfg(feature = "workflow-capture")]
use std::path::{Path, PathBuf};

#[cfg(feature = "workflow-capture")]
fn session_directory(value: &std::ffi::OsStr) -> std::io::Result<PathBuf> {
    let path = Path::new(value);
    if !path.is_absolute() || !path.is_dir() {
        return Err(std::io::Error::other(
            "Workflow directory must be an existing absolute directory",
        ));
    }
    if std::fs::symlink_metadata(path)?.file_type().is_symlink() {
        return Err(std::io::Error::other("Workflow directory cannot be a link"));
    }
    path.canonicalize()
}

#[cfg(feature = "workflow-capture")]
fn identifier(value: &str) -> std::io::Result<String> {
    let id = uuid::Uuid::parse_str(value)
        .map_err(|_| std::io::Error::other("Workflow isolation requires a UUID"))?;
    Ok(format!(
        "com.freemocap.skellyspeak.workflow.{}",
        id.simple()
    ))
}

#[cfg(feature = "workflow-capture")]
pub(super) fn configure<R: tauri::Runtime>(
    mut context: tauri::Context<R>,
) -> std::io::Result<tauri::Context<R>> {
    let Some(directory) = std::env::var_os("SKELLYSPEAK_WORKFLOW_DIR") else {
        if std::env::var_os("SKELLYSPEAK_WORKFLOW_ISOLATION").is_some() {
            return Err(std::io::Error::other("Workflow directory is missing"));
        }
        return Ok(context);
    };
    let directory = session_directory(&directory)?;
    let mode = match std::env::var("SKELLYSPEAK_WORKFLOW_MODE").as_deref() {
        Ok("capture") => crate::ai::transport::workflow::Mode::Capture,
        Ok("replay") => crate::ai::transport::workflow::Mode::Replay,
        _ => {
            return Err(std::io::Error::other(
                "Workflow mode must be capture or replay",
            ));
        }
    };
    if mode == crate::ai::transport::workflow::Mode::Replay
        && std::env::var_os("SKELLYSPEAK_WORKFLOW_ISOLATION").is_none()
    {
        return Err(std::io::Error::other(
            "Replay requires an isolated workspace",
        ));
    }
    crate::ai::transport::workflow::configure(&directory, mode)
        .map_err(|_| std::io::Error::other("Workflow transport configuration failed"))?;
    if mode == crate::ai::transport::workflow::Mode::Replay {
        crate::ai::transport::workflow::start()
            .map_err(|_| std::io::Error::other("Workflow replay could not start"))?;
    }
    if let Ok(address) = std::env::var("SKELLYSPEAK_WORKFLOW_UI_URL") {
        let url = tauri::Url::parse(&address)
            .map_err(|_| std::io::Error::other("Invalid workflow UI address"))?;
        if url.scheme() != "http"
            || url.host_str() != Some("127.0.0.1")
            || url.port().is_none()
            || !url.username().is_empty()
            || url.password().is_some()
        {
            return Err(std::io::Error::other(
                "Workflow UI requires a numeric loopback HTTP address",
            ));
        }
        context.config_mut().build.dev_url = Some(url);
    }
    if let Ok(isolation) = std::env::var("SKELLYSPEAK_WORKFLOW_ISOLATION") {
        context.config_mut().identifier = identifier(&isolation)?;
        for window in &mut context.config_mut().app.windows {
            window.data_directory = Some(directory.join("webview"));
            window.title = "SkellySpeak — workflow session".into();
        }
    }
    Ok(context)
}

#[cfg(feature = "workflow-capture")]
pub(super) fn prepare_workspace(destination: &Path) -> std::io::Result<()> {
    if !crate::ai::transport::workflow::is_replay() {
        return Ok(());
    }
    if destination.exists() {
        return Err(std::io::Error::other(
            "Replay workspace must not already exist",
        ));
    }
    let directory = std::env::var_os("SKELLYSPEAK_WORKFLOW_DIR")
        .ok_or_else(|| std::io::Error::other("Replay session directory is missing"))?;
    let source = session_directory(&directory)?.join("restored-workspace");
    let source = session_directory(source.as_os_str())?;
    std::fs::create_dir(destination)?;
    fn copy_files(source: &Path, destination: &Path, audio: bool) -> std::io::Result<()> {
        for entry in std::fs::read_dir(source)? {
            let entry = entry?;
            let kind = entry.file_type()?;
            let name = entry.file_name();
            if kind.is_dir() && !audio && name == "drill-audio" {
                std::fs::create_dir(destination.join(&name))?;
                copy_files(&entry.path(), &destination.join(name), true)?;
            } else if kind.is_file()
                && (audio
                    || [
                        "skellyspeak.sqlite3",
                        "skellyspeak.sqlite3-wal",
                        "skellyspeak.sqlite3-shm",
                        "skellyspeak.sqlite3-journal",
                    ]
                    .iter()
                    .any(|allowed| name == *allowed))
            {
                std::fs::copy(entry.path(), destination.join(name))?;
            } else {
                return Err(std::io::Error::other("Unexpected replay workspace member"));
            }
        }
        Ok(())
    }
    copy_files(&source, destination, false)
}

#[tauri::command]
pub(crate) fn workflow_control(
    app: tauri::AppHandle,
    state: tauri::State<'_, std::sync::Arc<super::Application>>,
    action: String,
) -> crate::model::Result<serde_json::Value> {
    #[cfg(feature = "workflow-capture")]
    {
        control(app, state.inner(), &action)
    }
    #[cfg(not(feature = "workflow-capture"))]
    {
        let _ = (app, state, action);
        Err(crate::model::AppError::new(
            crate::model::ErrorCode::Conflict,
            "Workflow recording requires a development capture build.",
        ))
    }
}

#[cfg(feature = "workflow-capture")]
fn control(
    app: tauri::AppHandle,
    state: &super::Application,
    action: &str,
) -> crate::model::Result<serde_json::Value> {
    use crate::model::{AppError, ErrorCode};
    use tauri::Manager;
    let invalid = |error: std::io::Error| AppError::new(ErrorCode::Storage, error.to_string());
    let directory = std::env::var_os("SKELLYSPEAK_WORKFLOW_DIR").ok_or_else(|| {
        AppError::new(
            ErrorCode::Conflict,
            "Launch an explicit workflow session first.",
        )
    })?;
    let directory = session_directory(&directory).map_err(invalid)?;
    match action {
        "local_timings_start" => {
            state.local_timings.start();
            Ok(serde_json::json!({"started":true}))
        }
        "local_timings_finish" => Ok(state.local_timings.finish()),
        "clock" => Ok(serde_json::json!({"micros":crate::ai::transport::workflow::clock_micros()})),
        "status" => Ok(
            serde_json::json!({"enabled":true,"isolated":std::env::var_os("SKELLYSPEAK_WORKFLOW_ISOLATION").is_some()}),
        ),
        "start" => {
            let busy = || {
                AppError::new(
                    ErrorCode::Conflict,
                    "Stop microphone capture before starting a workflow recording.",
                )
            };
            let capture = state.capture.try_lock().map_err(|_| busy())?;
            let listening = state.listening.try_lock().map_err(|_| busy())?;
            let microphone_test = state.microphone_test.try_lock().map_err(|_| busy())?;
            if capture.is_some() || listening.is_some() || microphone_test.is_some() {
                return Err(busy());
            }
            let store = state.lock()?;
            require_idle(&store.connection)?;
            let destination = directory.join("starting-state");
            std::fs::create_dir(&destination).map_err(invalid)?;
            let data = app.path().app_data_dir().map_err(|_| {
                AppError::new(
                    ErrorCode::Storage,
                    "Workflow workspace directory is unavailable.",
                )
            })?;
            let copy = crate::storage::factory_reset::export_to(
                &data,
                &destination,
                0,
                &store.ownership(),
            )?;
            crate::ai::transport::workflow::start()?;
            Ok(serde_json::json!({"workspace":copy}))
        }
        "stop" => {
            let store = state.lock()?;
            require_idle(&store.connection)?;
            crate::ai::transport::workflow::stop()?;
            Ok(serde_json::json!({"providerTape":directory.join("native-provider.json")}))
        }
        _ => Err(AppError::new(
            ErrorCode::Validation,
            "Unknown workflow action.",
        )),
    }
}

#[cfg(feature = "workflow-capture")]
fn require_idle(db: &rusqlite::Connection) -> crate::model::Result<()> {
    use crate::model::{AppError, ErrorCode};
    let mut groups = std::collections::BTreeMap::<String, Vec<String>>::new();
    let mut query = db.prepare("SELECT engine_id,run_id FROM graph_conversation_runs UNION SELECT engine_id,run_id FROM workspace_graph_runs")?;
    let rows = query.query_map([], |row| {
        Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
    })?;
    for row in rows {
        let (engine, run) = row?;
        groups.entry(engine).or_default().push(run);
    }
    for (engine, runs) in groups {
        let view = crate::ai::workspace_graph::inspection(db, &engine)?;
        for run in runs {
            let snapshot = view
                .snapshot(
                    &run,
                    crate::ai::graph::ExportLimits {
                        bytes: 4 * 1024 * 1024,
                        attempts: 4096,
                    },
                )
                .map_err(crate::ai::workspace_graph::error)?;
            if snapshot.nodes.values().any(|state| {
                use crate::ai::graph::Disposition;
                matches!(
                    state,
                    Disposition::Ready
                        | Disposition::Prepared
                        | Disposition::Running
                        | Disposition::Available
                )
            }) {
                return Err(AppError::new(
                    ErrorCode::Conflict,
                    "Wait for pending AI work to finish before starting or saving a workflow recording.",
                ));
            }
        }
    }
    Ok(())
}

#[cfg(all(test, feature = "workflow-capture"))]
mod tests {
    use super::*;

    #[test]
    fn isolated_identity_is_distinct_and_requires_a_uuid() {
        let id = identifier("d7d25a1f-75ad-4a70-9c2f-189760367212").unwrap();
        assert_eq!(
            id,
            "com.freemocap.skellyspeak.workflow.d7d25a1f75ad4a709c2f189760367212"
        );
        assert!(identifier("../com.freemocap.skellyspeak").is_err());
    }

    #[test]
    fn session_path_must_exist_and_be_absolute() {
        assert!(session_directory(std::ffi::OsStr::new("relative")).is_err());
        let directory = tempfile::tempdir().unwrap();
        assert_eq!(
            session_directory(directory.path().as_os_str()).unwrap(),
            directory.path().canonicalize().unwrap()
        );
        assert!(session_directory(directory.path().join("absent").as_os_str()).is_err());
    }
}
