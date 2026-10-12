//! Irreversible local reset.
//!
//! The reset erases secrets and workspace data while retaining the ownership lock,
//! and clears the webview's browsing data through WebView2, which owns its profile folder and may
//! hold files in it after this process exits. The log directory is still open while
//! this process runs, so it is recorded for the next launch, which clears it before
//! the log sink opens. A cleanup that fails again is reported on screen rather than
//! discarded.
use crate::ai::connections::credentials;
use crate::application::Application;
use crate::diagnostics;
use crate::model::AppError;
use crate::model::ErrorCode;
use crate::model::Result;
use crate::storage::store::Store;
use crate::storage::store::WORKSPACE_FILE;
use crate::storage::store::WORKSPACE_LOCK;
use crate::storage::store::WorkspaceOwnership;
use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use tauri::{AppHandle, Manager};

/// Only symbolic cleanup kinds are persisted; destinations come from configuration.
const PENDING: &str = "pending-cleanup";

fn storage_error(message: impl Into<String>) -> AppError {
    AppError::new(ErrorCode::Storage, message.into())
}

pub(crate) fn pending_path(directory: &Path) -> PathBuf {
    directory.join(PENDING)
}

/// Removes every entry in `directory`, leaving the pending record in place when the
/// caller still needs it to survive.
fn clear_directory(directory: &Path, keep: &[&str]) -> Result<()> {
    let metadata = match std::fs::symlink_metadata(directory) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(error) => {
            return Err(storage_error(format!(
                "Could not inspect local data: {error}"
            )));
        }
    };
    if !metadata.is_dir() || metadata.file_type().is_symlink() {
        return Err(storage_error(
            "Local data directory is not a real directory.",
        ));
    }
    for entry in std::fs::read_dir(directory)
        .map_err(|error| storage_error(format!("Could not read local data: {error}")))?
    {
        let entry =
            entry.map_err(|error| storage_error(format!("Could not read local data: {error}")))?;
        if keep.iter().any(|name| entry.file_name() == *name) {
            continue;
        }
        let path = entry.path();
        let kind = entry
            .file_type()
            .map_err(|error| storage_error(format!("Could not inspect local data: {error}")))?;
        if kind.is_dir() {
            std::fs::remove_dir_all(path)
        } else {
            std::fs::remove_file(path)
        }
        .map_err(|error| storage_error(format!("Could not erase local data: {error}")))?;
    }
    Ok(())
}

/// Written before reset touches secrets or data. No paths are accepted.
pub(crate) fn record_pending(directory: &Path) -> Result<()> {
    let temporary = directory.join(format!(".pending-cleanup-{}", uuid::Uuid::new_v4()));
    let write = (|| -> std::io::Result<()> {
        use std::io::Write;
        let mut options = std::fs::OpenOptions::new();
        options.write(true).create_new(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600);
        }
        let mut file = options.open(&temporary)?;
        file.write_all(b"logs\n")?;
        file.sync_all()?;
        std::fs::rename(&temporary, pending_path(directory))
    })();
    if let Err(error) = write {
        if temporary.exists() {
            std::fs::remove_file(&temporary)
                .map_err(|cleanup| storage_error(format!("Could not record cleanup: {error}; could not remove temporary record: {cleanup}")))?;
        }
        return Err(storage_error(format!("Could not record cleanup: {error}")));
    }
    Ok(())
}

fn pending(directory: &Path) -> Result<bool> {
    let path = pending_path(directory);
    let metadata = match std::fs::symlink_metadata(&path) {
        Ok(value) => value,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(false),
        Err(error) => {
            return Err(storage_error(format!(
                "Could not inspect cleanup record: {error}"
            )));
        }
    };
    if !metadata.is_file() || metadata.file_type().is_symlink() || metadata.len() > 32 {
        return Err(storage_error("Invalid cleanup record."));
    }
    let body = std::fs::read_to_string(path)
        .map_err(|error| storage_error(format!("Could not read cleanup record: {error}")))?;
    if body != "logs\n" {
        return Err(storage_error(
            "Invalid cleanup record; no directories were cleared.",
        ));
    }
    Ok(true)
}

pub(crate) fn clear_pending(directory: &Path) -> Result<()> {
    match std::fs::remove_file(pending_path(directory)) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(storage_error(format!(
            "Could not clear the cleanup record: {error}"
        ))),
    }
}

/// Finishes the cleanup a previous reset recorded.
///
/// Called before diagnostics opens its files. A directory that still cannot be
/// cleared keeps its record, so the
/// next launch tries again and the failure is reported until it succeeds.
pub(crate) fn finish_pending(directory: &Path, trusted_logs: &Path) -> Result<()> {
    let _ownership = WorkspaceOwnership::acquire(&directory.join(WORKSPACE_FILE))?;
    if !pending(directory)? {
        return Ok(());
    }
    // Never clear the workspace's stable lock through a misconfigured log root.
    if trusted_logs.exists() {
        let logs = trusted_logs
            .canonicalize()
            .map_err(|error| storage_error(error.to_string()))?;
        let data = directory
            .canonicalize()
            .map_err(|error| storage_error(error.to_string()))?;
        if data.starts_with(logs) {
            return Err(storage_error(
                "Log cleanup cannot contain the workspace directory.",
            ));
        }
    }
    // Development logs share a root with independently running servers and
    // launchers. Reset owns only the native process run directories.
    let mut keep = Vec::new();
    if trusted_logs.is_dir() {
        for entry in std::fs::read_dir(trusted_logs)
            .map_err(|error| storage_error(format!("Could not list logs: {error}")))?
        {
            let entry =
                entry.map_err(|error| storage_error(format!("Could not inspect logs: {error}")))?;
            let name = entry
                .file_name()
                .into_string()
                .map_err(|_| storage_error("Invalid log entry name."))?;
            let owned = name
                .strip_prefix("native-")
                .is_some_and(|id| uuid::Uuid::parse_str(id).is_ok());
            if !owned {
                keep.push(name);
            }
        }
    }
    clear_directory(
        trusted_logs,
        &keep.iter().map(String::as_str).collect::<Vec<_>>(),
    )
    .map_err(|error| storage_error(format!("Could not finish clearing logs: {error}")))?;
    clear_pending(directory)
}

fn credential_ids(store: &Store) -> Result<BTreeSet<String>> {
    let mut statement = store.connection.prepare(
        "SELECT hosted_credential_id FROM ai_config
         UNION SELECT custom_credential_id FROM ai_config
         UNION SELECT id FROM credential_cleanup",
    )?;
    let mut ids = store
        .credential_writes
        .iter()
        .cloned()
        .collect::<BTreeSet<_>>();
    for row in statement.query_map([], |row| row.get::<_, Option<String>>(0))? {
        if let Some(id) = row? {
            ids.insert(id);
        }
    }
    Ok(ids)
}

/// Erases every secret and workspace entry except the lock, active mobile logs and
/// record of what the next launch still has to clear. Split from the command so
/// the case that matters most
/// — a refused workspace, where no store exists — is covered by a test rather than
/// only by the running app. Credential removal is injected so a test never touches
/// the real keychain.
pub(crate) fn erase(
    data: &Path,
    _ownership: &WorkspaceOwnership,
    ids: &BTreeSet<String>,
    remove: &mut dyn FnMut(&str) -> Result<()>,
) -> Result<()> {
    for id in ids {
        remove(id)?;
    }
    // Mobile diagnostics live inside app data and remain open through shutdown.
    // The pending cleanup clears them before diagnostics initializes at launch.
    clear_directory(data, &[PENDING, WORKSPACE_LOCK, "logs"])
}

/// Caller holds workspace ownership and, for an open Store, its write mutex.
/// Publish only after the database, sidecars and editable configuration are copied.
// Audio referenced by the database is part of the same workspace snapshot.
// The caller holds Store's lock, so publication/pruning cannot race this copy.
fn copy_drill_audio(data: &Path, staging: &Path) -> Result<()> {
    let source = data.join("drill-audio");
    let metadata = match std::fs::symlink_metadata(&source) {
        Ok(value) => value,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(error) => {
            return Err(storage_error(format!(
                "Could not inspect drill audio: {error}"
            )));
        }
    };
    if !metadata.is_dir() || metadata.file_type().is_symlink() {
        return Err(storage_error("Drill audio must be a real directory."));
    }
    let destination = staging.join("drill-audio");
    std::fs::create_dir(&destination)
        .map_err(|error| storage_error(format!("Could not create audio backup: {error}")))?;
    for entry in std::fs::read_dir(source)
        .map_err(|error| storage_error(format!("Could not list drill audio: {error}")))?
    {
        let entry = entry
            .map_err(|error| storage_error(format!("Could not inspect drill audio: {error}")))?;
        let kind = entry
            .file_type()
            .map_err(|error| storage_error(format!("Could not inspect drill audio: {error}")))?;
        if !kind.is_file() || kind.is_symlink() {
            return Err(storage_error("Drill audio entries must be regular files."));
        }
        std::fs::copy(entry.path(), destination.join(entry.file_name()))
            .map_err(|error| storage_error(format!("Could not copy drill audio: {error}")))?;
    }
    Ok(())
}

pub(crate) fn export_to(
    data: &Path,
    destination: &Path,
    stamp: u64,
    _ownership: &WorkspaceOwnership,
) -> Result<PathBuf> {
    let token = uuid::Uuid::new_v4();
    let folder = destination.join(format!("skellyspeak-backup-{stamp}-{token}"));
    let staging = destination.join(format!(".skellyspeak-backup-{stamp}-{token}.partial"));
    std::fs::create_dir(&staging)
        .map_err(|error| storage_error(format!("Could not create backup directory: {error}")))?;
    let copy = (|| -> Result<()> {
        for suffix in ["", "-wal", "-shm", "-journal"] {
            let name = format!("{WORKSPACE_FILE}{suffix}");
            let source = data.join(&name);
            let metadata = match std::fs::symlink_metadata(&source) {
                Ok(value) => value,
                Err(error)
                    if error.kind() == std::io::ErrorKind::NotFound && !suffix.is_empty() =>
                {
                    continue;
                }
                Err(error) => {
                    return Err(storage_error(format!(
                        "Could not inspect workspace: {error}"
                    )));
                }
            };
            if !metadata.is_file() || metadata.file_type().is_symlink() {
                return Err(storage_error("Workspace files must be regular files."));
            }
            std::fs::copy(source, staging.join(name))
                .map_err(|error| storage_error(format!("Could not copy workspace: {error}")))?;
        }
        copy_drill_audio(data, &staging)?;
        std::fs::rename(&staging, &folder)
            .map_err(|error| storage_error(format!("Could not publish backup: {error}")))?;
        Ok(())
    })();
    if let Err(error) = copy {
        std::fs::remove_dir_all(&staging).map_err(|cleanup| {
            storage_error(format!(
                "{error}; could not remove incomplete backup: {cleanup}"
            ))
        })?;
        return Err(error);
    }
    Ok(folder)
}

/// Save a copy of the workspace to the Downloads folder: from Settings at any time,
/// and from the refusal screen before a reset. Returns the folder.
///
/// Every write goes through the store's lock, so holding it for the copy means no
/// write lands halfway through; the journal file is copied with the database. A
/// refused workspace must independently acquire the same process ownership lock.
#[tauri::command]
pub fn export_workspace(
    app: AppHandle,
    state: tauri::State<'_, std::sync::Arc<Application>>,
) -> Result<String> {
    let writes = state.store.lock().map_err(|_| {
        crate::diagnostics::failures::poisoned(storage_error("Local data is unavailable."))
    })?;
    let data = app
        .path()
        .app_data_dir()
        .map_err(|error| storage_error(format!("Could not locate local data: {error}")))?;
    let downloads = app.path().download_dir().map_err(|error| {
        storage_error(format!("Could not locate the Downloads folder: {error}"))
    })?;
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_err(|_| storage_error("The system clock is before 1970."))?
        .as_secs();
    let ownership = match writes.as_ref() {
        Some(store) => store.ownership(),
        None => WorkspaceOwnership::acquire(&data.join(WORKSPACE_FILE))?,
    };
    Ok(export_to(&data, &downloads, stamp, &ownership)?
        .display()
        .to_string())
}

#[tauri::command]
pub fn factory_reset(
    app: AppHandle,
    state: tauri::State<'_, std::sync::Arc<Application>>,
    confirmation: String,
) -> Result<()> {
    if confirmation != "DELETE" {
        return Err(AppError::new(
            ErrorCode::Validation,
            "Type DELETE to confirm deleting all local data.",
        ));
    }
    let data = app
        .path()
        .app_data_dir()
        .map_err(|error| storage_error(format!("Could not locate local data: {error}")))?;
    let _credential_operation = state.credential_operation()?;
    let mut stores = state.store.lock().map_err(|_| {
        crate::diagnostics::failures::poisoned(storage_error("Local data is unavailable."))
    })?;
    let ownership = match stores.as_ref() {
        Some(store) => store.ownership(),
        None => WorkspaceOwnership::acquire(&data.join(WORKSPACE_FILE))?,
    };
    diagnostics::log_root()?;
    record_pending(&data)?;
    // The identifiers normally live in the database. A refused workspace has no
    // database to read them from, so the index written before each secret is the
    // authority; when the workspace opens, both sources are erased from.
    let mut ids = credentials::indexed(&credentials::index_path(&data))?;
    if let Some(store) = stores.as_ref() {
        ids.extend(credential_ids(store)?);
    }
    for webview in app.webview_windows().values() {
        webview
            .clear_all_browsing_data()
            .map_err(|error| storage_error(format!("Could not clear browser data: {error}")))?;
    }
    state.clear_graph_reports();
    state.stop(AppError::new(
        ErrorCode::Internal,
        "Local data was deleted.",
    ));
    drop(stores.take());
    erase(&data, &ownership, &ids, &mut |id| credentials::remove(id))?;
    app.exit(0);
    Ok(())
}
#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    fn ownership(data: &Path) -> WorkspaceOwnership {
        WorkspaceOwnership::acquire(&data.join(WORKSPACE_FILE)).unwrap()
    }

    #[test]
    fn clears_nested_data_without_following_links() {
        let root = tempfile::tempdir().unwrap();
        let outside = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(root.path().join("nested")).unwrap();
        std::fs::write(root.path().join("nested/data"), "private").unwrap();
        std::fs::write(outside.path().join("keep"), "outside").unwrap();
        #[cfg(unix)]
        std::os::unix::fs::symlink(outside.path(), root.path().join("link")).unwrap();
        clear_directory(root.path(), &[]).unwrap();
        assert_eq!(std::fs::read_dir(root.path()).unwrap().count(), 0);
        assert!(outside.path().join("keep").exists());
    }

    #[test]
    fn reset_retains_active_mobile_logs_until_next_launch() {
        let data = tempfile::tempdir().unwrap();
        let logs = data.path().join("logs");
        std::fs::create_dir(&logs).unwrap();
        let run = logs.join(format!("native-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir(&run).unwrap();
        let log_path = run.join("native.jsonl");
        let mut log = std::fs::File::create(&log_path).unwrap();
        let guard = ownership(data.path());
        record_pending(data.path()).unwrap();
        std::fs::write(data.path().join(WORKSPACE_FILE), "workspace").unwrap();

        erase(data.path(), &guard, &BTreeSet::new(), &mut |_| Ok(())).unwrap();
        assert!(!data.path().join(WORKSPACE_FILE).exists());
        log.write_all(b"shutdown diagnostic\n").unwrap();
        log.flush().unwrap();
        assert_eq!(
            std::fs::read_to_string(&log_path).unwrap(),
            "shutdown diagnostic\n"
        );
        assert!(pending(data.path()).unwrap());

        drop(log);
        drop(guard);
        finish_pending(data.path(), &logs).unwrap();
        assert!(!log_path.exists());
        assert!(!pending(data.path()).unwrap());
        assert!(Store::open(&data.path().join(WORKSPACE_FILE)).is_ok());
    }

    #[test]
    fn refused_workspace_reset_retains_lock_and_reopens_fresh() {
        let data = tempfile::tempdir().unwrap();
        let logs = tempfile::tempdir().unwrap();
        let workspace = data.path().join(WORKSPACE_FILE);
        drop(Store::open(&workspace).unwrap());
        let connection = rusqlite::Connection::open(&workspace).unwrap();
        connection.pragma_update(None, "user_version", 8).unwrap();
        drop(connection);
        assert!(Store::open(&workspace).is_err());
        let guard = ownership(data.path());
        let index = credentials::index_path(data.path());
        credentials::remember(&index, "orphan-id").unwrap();
        record_pending(data.path()).unwrap();
        let run = logs.path().join(format!("native-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir(&run).unwrap();
        std::fs::write(run.join("native.jsonl"), "private fixture").unwrap();
        let mut removed = Vec::new();
        erase(
            data.path(),
            &guard,
            &credentials::indexed(&index).unwrap(),
            &mut |id| {
                removed.push(id.to_owned());
                Ok(())
            },
        )
        .unwrap();
        assert_eq!(removed, ["orphan-id"]);
        assert!(data.path().join(WORKSPACE_LOCK).exists());
        assert!(!workspace.exists());
        assert!(WorkspaceOwnership::acquire(&workspace).is_err());
        drop(guard);
        finish_pending(data.path(), logs.path()).unwrap();
        assert!(!pending(data.path()).unwrap());
        assert_eq!(std::fs::read_dir(logs.path()).unwrap().count(), 0);
        assert!(
            Store::open(&workspace)
                .unwrap()
                .snapshot()
                .unwrap()
                .contacts
                .is_empty()
        );
    }

    #[test]
    fn pending_rejects_unrelated_and_malformed_records_before_deletion() {
        let data = tempfile::tempdir().unwrap();
        let logs = tempfile::tempdir().unwrap();
        let unrelated = tempfile::tempdir().unwrap();
        for root in [logs.path(), unrelated.path()] {
            std::fs::write(root.join("sentinel"), "keep").unwrap();
        }
        for body in [
            format!("{}\n", unrelated.path().display()),
            "logs\nunknown\n".into(),
            "".into(),
            "logs\nlogs\n".into(),
        ] {
            std::fs::write(pending_path(data.path()), body).unwrap();
            assert!(finish_pending(data.path(), logs.path()).is_err());
            assert!(logs.path().join("sentinel").exists());
            assert!(unrelated.path().join("sentinel").exists());
        }
    }

    #[test]
    fn cleanup_failure_retains_record_and_partial_cleanup_retries() {
        let data = tempfile::tempdir().unwrap();
        let root = tempfile::tempdir().unwrap();
        let logs = root.path().join("logs");
        std::fs::write(&logs, "not a directory").unwrap();
        record_pending(data.path()).unwrap();
        assert!(finish_pending(data.path(), &logs).is_err());
        assert!(pending(data.path()).unwrap());
        std::fs::remove_file(&logs).unwrap();
        finish_pending(data.path(), &logs).unwrap();
        assert!(!pending(data.path()).unwrap());
        finish_pending(data.path(), &logs).unwrap();
    }

    #[test]
    fn reset_cleans_only_native_runs_and_leaves_running_server_logs_alone() {
        let data = tempfile::tempdir().unwrap();
        let logs = tempfile::tempdir().unwrap();
        let native = logs.path().join(format!("native-{}", uuid::Uuid::new_v4()));
        let server = logs.path().join("server-running");
        std::fs::create_dir(&native).unwrap();
        std::fs::create_dir(&server).unwrap();
        std::fs::write(native.join("native.jsonl"), "app log").unwrap();
        let server_file = server.join("server-logging.jsonl");
        std::fs::write(&server_file, "server log").unwrap();
        let mut options = std::fs::OpenOptions::new();
        options.read(true);
        #[cfg(windows)]
        {
            use std::os::windows::fs::OpenOptionsExt;
            options.share_mode(0);
        }
        let open_server = options.open(&server_file).unwrap();
        record_pending(data.path()).unwrap();
        finish_pending(data.path(), logs.path()).unwrap();
        assert!(!native.exists());
        assert!(!pending(data.path()).unwrap());
        drop(open_server);
        assert_eq!(std::fs::read_to_string(server_file).unwrap(), "server log");
    }

    #[test]
    fn configured_logs_cannot_erase_workspace_ownership() {
        let data = tempfile::tempdir().unwrap();
        record_pending(data.path()).unwrap();
        assert!(finish_pending(data.path(), data.path()).is_err());
        assert!(data.path().join(WORKSPACE_LOCK).exists());
        assert!(pending(data.path()).unwrap());
    }

    #[test]
    fn live_workspace_prevents_deferred_cleanup() {
        let data = tempfile::tempdir().unwrap();
        let logs = tempfile::tempdir().unwrap();
        let _store = Store::open(&data.path().join(WORKSPACE_FILE)).unwrap();
        record_pending(data.path()).unwrap();
        std::fs::write(logs.path().join("sentinel"), "keep").unwrap();
        assert_eq!(
            finish_pending(data.path(), logs.path()).unwrap_err().code,
            ErrorCode::Conflict
        );
        assert!(logs.path().join("sentinel").exists());
    }

    #[test]
    fn backup_is_unique_with_no_stale_sidecars_and_reopens_sqlite() {
        let data = tempfile::tempdir().unwrap();
        let downloads = tempfile::tempdir().unwrap();
        let store = Store::open(&data.path().join(WORKSPACE_FILE)).unwrap();
        store
            .connection
            .pragma_update(None, "journal_mode", "WAL")
            .unwrap();
        store.connection.execute_batch("CREATE TABLE backup_probe (value TEXT); INSERT INTO backup_probe VALUES ('first');").unwrap();
        std::fs::create_dir(data.path().join("drill-audio")).unwrap();
        std::fs::write(
            data.path().join("drill-audio/attempt.wav"),
            b"retained audio",
        )
        .unwrap();
        let guard = store.ownership();
        let first = export_to(data.path(), downloads.path(), 42, &guard).unwrap();
        store
            .connection
            .execute("UPDATE backup_probe SET value = 'second'", [])
            .unwrap();
        store
            .connection
            .execute_batch("PRAGMA wal_checkpoint(TRUNCATE)")
            .unwrap();
        drop(store);
        let second = export_to(data.path(), downloads.path(), 42, &guard).unwrap();
        assert_ne!(first, second);
        assert!(!second.join(format!("{WORKSPACE_FILE}-wal")).exists());
        for (folder, expected) in [(first, "first"), (second, "second")] {
            assert_eq!(
                std::fs::read(folder.join("drill-audio/attempt.wav")).unwrap(),
                b"retained audio"
            );
            let copy = rusqlite::Connection::open(folder.join(WORKSPACE_FILE)).unwrap();
            assert_eq!(
                copy.query_row("SELECT value FROM backup_probe", [], |row| row
                    .get::<_, String>(0))
                    .unwrap(),
                expected
            );
            assert_eq!(
                copy.query_row("PRAGMA integrity_check", [], |row| row.get::<_, String>(0))
                    .unwrap(),
                "ok"
            );
        }
    }

    #[test]
    fn backup_contains_workspace_not_obsolete_teaching_content() {
        let data = tempfile::tempdir().unwrap();
        let downloads = tempfile::tempdir().unwrap();
        let guard = ownership(data.path());
        std::fs::write(data.path().join(WORKSPACE_FILE), "database").unwrap();
        std::fs::create_dir(data.path().join("config")).unwrap();
        std::fs::write(data.path().join("config/old.yaml"), "obsolete").unwrap();
        let backup = export_to(data.path(), downloads.path(), 1, &guard).unwrap();
        assert!(!backup.join("config").exists());
        assert!(data.path().join("config/old.yaml").exists());
    }

    #[test]
    fn incomplete_backup_is_not_published_or_retained() {
        let data = tempfile::tempdir().unwrap();
        let downloads = tempfile::tempdir().unwrap();
        let guard = ownership(data.path());
        assert!(export_to(data.path(), downloads.path(), 1, &guard).is_err());
        std::fs::write(data.path().join(WORKSPACE_FILE), "database").unwrap();
        std::fs::create_dir(data.path().join(format!("{WORKSPACE_FILE}-wal"))).unwrap();
        assert!(export_to(data.path(), downloads.path(), 1, &guard).is_err());
        assert_eq!(std::fs::read_dir(downloads.path()).unwrap().count(), 0);
        std::fs::remove_dir(data.path().join(format!("{WORKSPACE_FILE}-wal"))).unwrap();
        std::fs::create_dir_all(data.path().join("drill-audio/unexpected-directory")).unwrap();
        assert!(export_to(data.path(), downloads.path(), 1, &guard).is_err());
        assert_eq!(std::fs::read_dir(downloads.path()).unwrap().count(), 0);
        #[cfg(unix)]
        {
            std::fs::remove_dir(data.path().join("drill-audio/unexpected-directory")).unwrap();
            std::os::unix::fs::symlink(
                data.path().join(WORKSPACE_FILE),
                data.path().join("drill-audio/linked.wav"),
            )
            .unwrap();
            assert!(export_to(data.path(), downloads.path(), 1, &guard).is_err());
            assert_eq!(std::fs::read_dir(downloads.path()).unwrap().count(), 0);
        }
    }

    #[test]
    fn child_process_cannot_acquire_owned_workspace() {
        if let Some(path) = std::env::var_os("SKELLY_RESET_LOCK_TEST_DIRECTORY") {
            let error = match WorkspaceOwnership::acquire(&PathBuf::from(path).join(WORKSPACE_FILE))
            {
                Ok(_) => panic!("child acquired workspace during reset"),
                Err(error) => error,
            };
            assert_eq!(error.code, ErrorCode::Conflict);
            return;
        }
        let data = tempfile::tempdir().unwrap();
        let store = Store::open(&data.path().join(WORKSPACE_FILE)).unwrap();
        let guard = store.ownership();
        let check = || {
            let result = std::process::Command::new(std::env::current_exe().unwrap())
                .args([
                    "--exact",
                    "factory_reset::tests::child_process_cannot_acquire_owned_workspace",
                    "--nocapture",
                ])
                .env("SKELLY_RESET_LOCK_TEST_DIRECTORY", data.path())
                .output()
                .unwrap();
            assert!(
                result.status.success(),
                "{}",
                String::from_utf8_lossy(&result.stderr)
            );
        };
        check();
        drop(store);
        erase(data.path(), &guard, &BTreeSet::new(), &mut |_| Ok(())).unwrap();
        check();
        drop(guard);
        assert!(Store::open(&data.path().join(WORKSPACE_FILE)).is_ok());
    }
}
