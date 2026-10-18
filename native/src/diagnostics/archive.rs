//! Export only structured diagnostic files, never the workspace or process output.
use crate::model::{AppError, ErrorCode, Result};
use std::{
    fs,
    io::{Read, Write},
    path::{Path, PathBuf},
};
use zip::{ZipWriter, write::SimpleFileOptions};
fn failure() -> AppError {
    AppError::new(ErrorCode::Storage, "Could not save diagnostic logs.")
}
fn io(error: std::io::Error, stage: &str) -> AppError {
    super::response::io_context(&error, stage, failure())
}
fn is_log(name: &str) -> bool {
    matches!(
        name,
        "native.jsonl" | "diagnostics.jsonl" | "execution-diagnostics.jsonl"
    ) || name
        .strip_prefix("native-")
        .and_then(|s| s.strip_suffix(".manifest.json"))
        .is_some_and(|s| !s.is_empty() && s.bytes().all(|b| b.is_ascii_digit()))
}
fn files(root: &Path) -> Result<Vec<PathBuf>> {
    let mut files = Vec::new();
    for entry in fs::read_dir(root).map_err(|e| io(e, "export_list"))? {
        let entry = entry.map_err(|e| io(e, "export_entry"))?;
        let kind = entry.file_type().map_err(|e| io(e, "export_file_type"))?;
        let name = entry.file_name();
        let name = name.to_string_lossy();
        if kind.is_symlink() {
            return Err(failure().with_diagnostics(
                serde_json::json!({"stage":"export_list","reason":"symlink_refused"}),
            ));
        }
        if kind.is_file() && is_log(&name) {
            files.push(entry.path());
        } else if kind.is_dir()
            && ["native-", "app-", "process-"]
                .iter()
                .any(|p| name.starts_with(p))
        {
            for child in fs::read_dir(entry.path()).map_err(|e| io(e, "export_run_list"))? {
                let child = child.map_err(|e| io(e, "export_run_entry"))?;
                if !is_log(&child.file_name().to_string_lossy()) {
                    continue;
                }
                if !child
                    .file_type()
                    .map_err(|e| io(e, "export_file_type"))?
                    .is_file()
                {
                    return Err(failure().with_diagnostics(
                        serde_json::json!({"stage":"export_file_type","reason":"not_regular_file"}),
                    ));
                }
                files.push(child.path());
            }
        }
    }
    files.sort();
    if files.is_empty() {
        return Err(failure().with_diagnostics(
            serde_json::json!({"stage":"export_list","reason":"no_structured_logs"}),
        ));
    }
    Ok(files)
}
pub fn save(root: &Path, destination: &Path) -> Result<PathBuf> {
    let root = root.canonicalize().map_err(|e| io(e, "export_root"))?;
    let files = files(&root)?;
    let path = destination.join(format!("skellyspeak-logs-{}.zip", uuid::Uuid::new_v4()));
    let mut options = fs::OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let file = options.open(&path).map_err(|e| io(e, "export_create"))?;
    let result = write(&root, files, file);
    if let Err(mut error) = result {
        if let Err(cleanup) = fs::remove_file(&path)
            && let Some(v) = error.diagnostics.as_mut()
        {
            v["cleanup_error"] =
                super::response::error_metadata(&io(cleanup, "export_cleanup"), &[]);
        }
        return Err(error);
    }
    Ok(path)
}
fn write(root: &Path, files: Vec<PathBuf>, file: fs::File) -> Result<()> {
    let mut zip = ZipWriter::new(file);
    let options = SimpleFileOptions::default().compression_method(zip::CompressionMethod::Stored);
    let zip_error = |error: zip::result::ZipError| {
        super::failures::platform(&error, "export_zip", &[], failure())
    };
    zip.start_file("manifest.json", options)
        .map_err(zip_error)?;
    let manifest = serde_json::json!({"formatVersion":1,"appVersion":env!("CARGO_PKG_VERSION"),"platform":std::env::consts::OS,"logFileCount":files.len(),"diagnosticPolicyVersion":1,"snapshot":"Structured logs and redacted execution history; active last log records may be incomplete"});
    zip.write_all(manifest.to_string().as_bytes())
        .map_err(|e| io(e, "export_manifest"))?;
    for path in files {
        let mut open = fs::OpenOptions::new();
        open.read(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            open.custom_flags(libc::O_NOFOLLOW);
        }
        let file = open.open(&path).map_err(|e| io(e, "export_read"))?;
        let length = file.metadata().map_err(|e| io(e, "export_metadata"))?.len();
        let relative = path
            .strip_prefix(root)
            .expect("collected inside root")
            .to_string_lossy()
            .replace('\\', "/");
        zip.start_file(format!("logs/{relative}"), options)
            .map_err(zip_error)?;
        let copied =
            std::io::copy(&mut file.take(length), &mut zip).map_err(|e| io(e, "export_copy"))?;
        if copied != length {
            return Err(failure().with_diagnostics(serde_json::json!({"stage":"export_copy","reason":"log_shrank","expected_bytes":length,"copied_bytes":copied})));
        }
    }
    zip.finish()
        .map_err(zip_error)?
        .sync_all()
        .map_err(|e| io(e, "export_flush"))?;
    Ok(())
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn exported_zip_preserves_failure_and_receipt_without_private_files() {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path().join("logs");
        let run = root.join("native-1-2");
        fs::create_dir_all(&run).unwrap();
        let error = AppError::new(ErrorCode::Validation,"Quote source-canary rejected at cards[0].quote; api_key=secret-canary")
            .with_diagnostics(serde_json::json!({"stage":"reply_explanations_validation","path":"cards[0].quote","expected":"exact source quote"}));
        let event = super::super::response::retained_with_private(
            Some(&serde_json::json!({"request_id":"req-42","usage":{"tokens":352}})),
            Some(&error),
            &["source-canary"],
        )
        .unwrap();
        fs::write(run.join("native.jsonl"), &event).unwrap();
        for name in [
            "workspace.sqlite",
            "credentials.index",
            "audio.wav",
            "stderr.jsonl",
        ] {
            fs::write(root.join(name), "private-file-canary").unwrap();
        }
        let path = save(&root, temp.path()).unwrap();
        let mut archive = zip::ZipArchive::new(fs::File::open(path).unwrap()).unwrap();
        assert_eq!(archive.len(), 2);
        let mut stored = String::new();
        archive
            .by_name("logs/native-1-2/native.jsonl")
            .unwrap()
            .read_to_string(&mut stored)
            .unwrap();
        for kept in [
            "cards[0].quote",
            "exact source quote",
            "req-42",
            "352",
            "rejected",
        ] {
            assert!(stored.contains(kept));
        }
        for secret in ["source-canary", "secret-canary", "private-file-canary"] {
            assert!(!stored.contains(secret));
        }
    }
    #[test]
    fn exported_zip_includes_retained_execution_diagnostics() {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path().join("logs");
        fs::create_dir(&root).unwrap();
        fs::write(root.join("native.jsonl"), "{}\n").unwrap();
        fs::write(
            root.join("execution-diagnostics.jsonl"),
            "{\"code\":\"execution_evidence\",\"request_id\":\"req-speech-failed\"}\n",
        )
        .unwrap();
        let path = save(&root, temp.path()).unwrap();
        let mut archive = zip::ZipArchive::new(fs::File::open(path).unwrap()).unwrap();
        let mut stored = String::new();
        archive
            .by_name("logs/execution-diagnostics.jsonl")
            .unwrap()
            .read_to_string(&mut stored)
            .unwrap();
        assert!(stored.contains("req-speech-failed"));
    }
    #[cfg(unix)]
    #[test]
    fn refuses_links_into_private_files() {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path().join("logs");
        fs::create_dir(&root).unwrap();
        fs::write(temp.path().join("secret"), "private").unwrap();
        std::os::unix::fs::symlink(temp.path().join("secret"), root.join("native.jsonl")).unwrap();
        assert!(save(&root, temp.path()).is_err());
    }
}
