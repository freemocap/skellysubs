//! Durable, redacted diagnostics; the bounded ring is only a recent-read view.
#[cfg(any(not(target_os = "android"), test))]
mod archive;
pub(crate) mod failures;
mod policy;
pub(crate) mod response;
pub(crate) mod sharing;
pub(crate) mod structured;
use crate::model::AppError;
use crate::model::ErrorCode;
use crate::model::Result;
use serde::{Deserialize, Serialize};
use std::{
    collections::VecDeque,
    fs::{File, OpenOptions},
    io::Write,
    path::{Path, PathBuf},
    sync::{Mutex, OnceLock},
    time::{SystemTime, UNIX_EPOCH},
};

const CAPACITY: usize = 256;
const MICROPHONE_LOG_TARGET: &str = "skellyspeak_core::speech::recording::audio";

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum DiagnosticContext {
    Application,
    Conversation,
    Speech,
    Microphone,
    Settings,
    Audio,
    Navigation,
    Other,
}
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum DiagnosticCode {
    UiEvent,
    UiFault,
    NativeCommandFailed,
    UnhandledError,
    UnhandledRejection,
    DiagnosticBridgeFailed,
}
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum DiagnosticLevel {
    Debug,
    Info,
    Warn,
    Error,
}
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum DiagnosticCause {
    CustomTranscriptionUnconfigured,
    PlaybackDenied,
    PlaybackFailed,
    NetworkFailure,
    ResizeObserverLoop,
    ResourceLoadFailed,
    TypeError,
    ReferenceError,
    SyntaxError,
    AbortError,
    Unknown,
}
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum DiagnosticEvent {
    Console,
    IpcStarted,
    IpcSucceeded,
    IpcFailed,
    SettingsOpened,
    SettingsLoaded,
    SettingsSaving,
    SpeechRequested,
    SpeechSuppressed,
    SpeechPlaying,
    SpeechFinished,
    MicrophoneAutosend,
    MicrophoneEmpty,
    ApplicationMounted,
    LanguageRegistryLoaded,
    Other,
}
include!(concat!(env!("OUT_DIR"), "/diagnostic_commands.rs"));

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct FrontendDiagnostic {
    #[serde(default)]
    pub diagnostics: Option<serde_json::Value>,
    pub context: DiagnosticContext,
    pub code: DiagnosticCode,
    pub level: Option<DiagnosticLevel>,
    pub native_code: Option<ErrorCode>,
    pub fault_id: Option<u32>,
    pub command: Option<DiagnosticCommand>,
    pub cause: Option<DiagnosticCause>,
    pub event_name: Option<DiagnosticEvent>,
    pub redacted_args: Option<u32>,
}
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DiagnosticRecord {
    pub sequence: u64,
    pub recorded_at_ms: u64,
    pub event: FrontendDiagnostic,
}
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DiagnosticReceipt {
    pub sequence: u64,
    pub recorded_at_ms: u64,
}

struct Buffer {
    records: VecDeque<DiagnosticRecord>,
    sequence: u64,
}
impl Buffer {
    fn new() -> Self {
        Self {
            records: VecDeque::new(),
            sequence: 0,
        }
    }
    fn push(&mut self, event: FrontendDiagnostic, recorded_at_ms: u64) -> Result<DiagnosticRecord> {
        let sequence = self.sequence.checked_add(1).ok_or_else(unavailable)?;
        let record = DiagnosticRecord {
            sequence,
            recorded_at_ms,
            event,
        };
        self.sequence = sequence;
        if self.records.len() == CAPACITY {
            self.records.pop_front();
        }
        self.records.push_back(record.clone());
        Ok(record)
    }
    fn after(&self, sequence: Option<u64>) -> Vec<DiagnosticRecord> {
        self.records
            .iter()
            .filter(|r| sequence.is_none_or(|after| r.sequence > after))
            .cloned()
            .collect()
    }
}
fn unavailable() -> AppError {
    AppError::new(ErrorCode::Internal, "Native diagnostics are unavailable.")
}
/// Stage alone is useful for invariant failures; IO failures also retain OS identity.
fn unavailable_at(stage: &'static str) -> AppError {
    eprintln!("Native diagnostics setup failed at {stage}.");
    AppError::new(
        ErrorCode::Internal,
        format!("Native diagnostics could not initialize ({stage})."),
    )
}
fn unavailable_io(stage: &'static str, cause: std::io::Error) -> AppError {
    let error = response::io_context(&cause, stage, unavailable_at(stage));
    eprintln!(
        "Native diagnostics IO failure: {}",
        error.diagnostics.as_ref().unwrap()
    );
    error
}
fn buffer() -> &'static Mutex<Buffer> {
    static BUFFER: OnceLock<Mutex<Buffer>> = OnceLock::new();
    BUFFER.get_or_init(|| Mutex::new(Buffer::new()))
}

fn timestamp() -> Result<u64> {
    let value = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|_| unavailable())?
        .as_millis();
    u64::try_from(value).map_err(|_| unavailable())
}

struct FileSink {
    directory: PathBuf,
    frontend: File,
    native: File,
}
fn private_file(path: &Path) -> std::io::Result<File> {
    let mut options = OpenOptions::new();
    options.create_new(true).append(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    options.open(path)
}
impl FileSink {
    fn open_run(root: &Path) -> Result<Self> {
        // A launcher session can restart the native process repeatedly. Its
        // inherited log root is shared, but each process owns fresh stream files.
        Self::open(&root.join(format!("native-{}", uuid::Uuid::new_v4())))
    }

    fn open(directory: &Path) -> Result<Self> {
        // Desktop paths must not traverse user-controlled symlinks. Android's
        // app-private path is supplied by the OS but conventionally includes the
        // system-owned /data/user/0 alias, so rejecting every ancestor there would
        // reject every valid app-data directory. `prepare_private_directory` still
        // rejects a symlink at the final private directory on every Unix platform.
        #[cfg(not(target_os = "android"))]
        for ancestor in directory.ancestors() {
            if let Ok(metadata) = std::fs::symlink_metadata(ancestor)
                && metadata.file_type().is_symlink()
            {
                return Err(unavailable_at("path_validation"));
            }
        }
        crate::storage::store::prepare_private_directory(directory)
            .map_err(|cause| unavailable_io("directory", cause))?;
        let frontend = private_file(&directory.join("diagnostics.jsonl"))
            .map_err(|cause| unavailable_io("frontend_file", cause))?;
        let native = private_file(&directory.join("native.jsonl"))
            .map_err(|cause| unavailable_io("native_file", cause))?;
        let mut manifest =
            private_file(&directory.join(format!("native-{}.manifest.json", std::process::id())))
                .map_err(|cause| unavailable_io("manifest_file", cause))?;
        serde_json::to_writer(
            &mut manifest,
            &serde_json::json!({
                "version": 1, "pid": std::process::id(), "startedAtMs": timestamp()?,
                "source": "native", "streams": ["diagnostics.jsonl", "native.jsonl"],
                "retention": "no_automatic_deletion", "content": "structured_allowlisted_metadata",
                "appVersion": env!("CARGO_PKG_VERSION"), "platform": std::env::consts::OS,
                "architecture": std::env::consts::ARCH, "debugBuild": cfg!(debug_assertions),
                "appImage": std::env::var_os("APPIMAGE").is_some(),
            }),
        )
        .map_err(|cause| response::json_context(&cause, "manifest_encoding", unavailable()))?;
        manifest
            .write_all(b"\n")
            .and_then(|_| manifest.flush())
            .map_err(|cause| unavailable_io("manifest_write", cause))?;
        Ok(Self {
            directory: directory.to_owned(),
            frontend,
            native,
        })
    }
    fn append(&mut self, source: &str, event: &serde_json::Value) -> Result<()> {
        let record = serde_json::json!({
            "recordedAtMs": timestamp()?, "run": self.directory.file_name().and_then(|n| n.to_str()),
            "pid": std::process::id(), "source": source, "event": event,
        });
        let mut bytes = serde_json::to_vec(&record).map_err(|cause| {
            response::json_context(&cause, "log_record_encoding", unavailable())
        })?;
        bytes.push(b'\n');
        let file = if source == "frontend" {
            &mut self.frontend
        } else {
            &mut self.native
        };
        file.write_all(&bytes)
            .and_then(|_| file.flush())
            .map_err(|cause| response::io_context(&cause, "diagnostic_append", unavailable()))
    }
}
static SINK: OnceLock<Mutex<FileSink>> = OnceLock::new();
static LOG_ROOT: OnceLock<PathBuf> = OnceLock::new();
fn sink() -> Result<&'static Mutex<FileSink>> {
    SINK.get().ok_or_else(unavailable)
}

/// Root calls this during setup before opening application state. No deletion/rotation.
/// Resolve cleanup destinations from the same trusted configuration used by the
/// sink, before initialization opens any files.
pub(crate) fn configured_root(fallback_root: &Path) -> Result<PathBuf> {
    let custom = std::env::var_os("SKELLYSPEAK_LOG_RUN_DIR");
    Ok(match custom.as_ref() {
        Some(value) => {
            let path = PathBuf::from(value);
            if !path.is_absolute() {
                return Err(unavailable_at("custom_root"));
            }
            path
        }
        None => {
            if cfg!(all(
                debug_assertions,
                not(any(target_os = "android", target_os = "ios"))
            )) {
                Path::new(env!("CARGO_MANIFEST_DIR"))
                    .parent()
                    .ok_or_else(unavailable)?
                    .join(".local/logs")
            } else {
                fallback_root.to_owned()
            }
        }
    })
}

pub fn initialize(fallback_root: &Path) -> Result<PathBuf> {
    if let Some(sink) = SINK.get() {
        return Ok(sink
            .lock()
            .map_err(|_| crate::diagnostics::failures::poisoned(unavailable()))?
            .directory
            .clone());
    }
    let root = configured_root(fallback_root)?;
    let mut output = FileSink::open_run(&root)?;
    let directory = output.directory.clone();
    output.append("native", &serde_json::json!({"code":"logging_initialized"}))?;
    LOG_ROOT
        .set(root)
        .map_err(|_| unavailable_at("root_registration"))?;
    SINK.set(Mutex::new(output))
        .map_err(|_| unavailable_at("sink_registration"))?;
    {
        log::set_logger(&NATIVE_LOGGER)
            .map_err(|_| unavailable_at("native_logger_already_installed"))?;
        log::set_max_level(log::LevelFilter::Info);
    }
    // Panic messages explain the failure; redact sensitive spans before persistence.
    std::panic::set_hook(Box::new(|info| {
        let message = info
            .payload()
            .downcast_ref::<String>()
            .map(String::as_str)
            .or_else(|| info.payload().downcast_ref::<&str>().copied())
            .map(|text| response::scrub(text, &[]))
            .unwrap_or_else(|| "[omitted: non-string panic payload]".into());
        let event = serde_json::json!({"code":"panic", "message":message, "line":info.location().map(|l| l.line()),
            "file":info.location().map(|l| l.file().rsplit("/src/").next().unwrap_or(l.file())),
            "contentRedacted":true});
        if let Err(error) = append_native(&event) {
            fallback("panic_persistence", &event, &error);
        }
        eprintln!("{}", event);
    }));
    Ok(directory)
}
/// Nonrecursive emergency channel when the primary sink fails.
fn fallback(stage: &str, event: &serde_json::Value, error: &AppError) {
    eprintln!(
        "{}",
        serde_json::json!({"stage":stage,"event":response::metadata(event, &[]),"sink_error":response::error_metadata(error, &[])})
    );
}
fn append_native(event: &serde_json::Value) -> Result<()> {
    sink()?
        .lock()
        .map_err(|_| crate::diagnostics::failures::poisoned(unavailable()))?
        .append("native", event)
}

pub fn log_root() -> Result<PathBuf> {
    LOG_ROOT.get().cloned().ok_or_else(unavailable)
}

/// Authored diagnostic code with numerical metrics only; no arbitrary error body.
pub fn native_event(code: &'static str, metrics: &[(&'static str, u64)]) {
    let event = serde_json::json!({"code":code, "metrics":metrics});
    if let Err(error) = append_native(&event) {
        fallback("native_event_persistence", &event, &error);
    }
}
struct NativeLogger;
static NATIVE_LOGGER: NativeLogger = NativeLogger;
impl log::Log for NativeLogger {
    fn enabled(&self, metadata: &log::Metadata<'_>) -> bool {
        metadata.level() <= log::Level::Warn
            || (metadata.level() == log::Level::Info && metadata.target() == MICROPHONE_LOG_TARGET)
    }
    fn log(&self, record: &log::Record<'_>) {
        if !self.enabled(record.metadata()) {
            return;
        }
        // Target/module may be caller-controlled in dependencies: only local microphone
        // target receives an authored category; arbitrary payloads are never persisted.
        let code = if record.target() == MICROPHONE_LOG_TARGET {
            "microphone_log"
        } else {
            "native_log"
        };
        let mut event = serde_json::json!({"code":code, "level":record.level().as_str(),
            "line":record.line(), "file":record.file().map(|p| p.rsplit("/src/").next().unwrap_or(p)), "module":record.module_path(), "contentRedacted":true});
        if record.level() <= log::Level::Warn {
            event["message"] = serde_json::json!(response::scrub(&record.args().to_string(), &[]));
            event["redaction"] = serde_json::json!("sensitive spans removed");
        }
        if record.target() == MICROPHONE_LOG_TARGET {
            let message = record.args().to_string();
            if let Some(rate) = message
                .strip_prefix("[mic] capture started at ")
                .and_then(|s| s.strip_suffix("Hz"))
                .and_then(|s| s.parse::<u32>().ok())
            {
                event["code"] = serde_json::json!("microphone_capture_started");
                event["sampleRate"] = serde_json::json!(rate);
            } else if let Some((samples, bytes)) = message
                .strip_prefix("[mic] capture finished: ")
                .and_then(|s| s.strip_suffix(" bytes of WAV"))
                .and_then(|s| s.split_once(" samples, "))
                .and_then(|(a, b)| Some((a.parse::<u64>().ok()?, b.parse::<u64>().ok()?)))
            {
                event["code"] = serde_json::json!("microphone_capture_finished");
                event["samples"] = serde_json::json!(samples);
                event["wavBytes"] = serde_json::json!(bytes);
            }
        }
        if let Err(error) = append_native(&event) {
            fallback("native_log_persistence", &event, &error);
        }
    }
    fn flush(&self) {} // Each append is flushed before returning.
}

fn sanitize_frontend(event: &mut FrontendDiagnostic) {
    event.diagnostics = event
        .diagnostics
        .as_ref()
        .map(|v| response::metadata(v, &[]));
}

#[tauri::command]
pub fn record_frontend_diagnostic(mut event: FrontendDiagnostic) -> Result<DiagnosticReceipt> {
    sanitize_frontend(&mut event);
    let timestamp = timestamp()?;
    let mut buffer = buffer()
        .lock()
        .map_err(|_| crate::diagnostics::failures::poisoned(unavailable()))?;
    // Commit to the file before publishing to the recent-read ring or acknowledging.
    let sequence = buffer.sequence.checked_add(1).ok_or_else(unavailable)?;
    let record = DiagnosticRecord {
        sequence,
        recorded_at_ms: timestamp,
        event,
    };
    let mut sink = sink()?
        .lock()
        .map_err(|_| crate::diagnostics::failures::poisoned(unavailable()))?;
    sink.append(
        "frontend",
        &serde_json::to_value(&record).map_err(|cause| {
            response::json_context(&cause, "frontend_record_encoding", unavailable())
        })?,
    )?;
    let record = buffer.push(record.event, timestamp)?;
    Ok(DiagnosticReceipt {
        sequence: record.sequence,
        recorded_at_ms: record.recorded_at_ms,
    })
}

#[tauri::command]
pub fn read_frontend_diagnostics(after_sequence: Option<u64>) -> Result<Vec<DiagnosticRecord>> {
    Ok(buffer()
        .lock()
        .map_err(|_| crate::diagnostics::failures::poisoned(unavailable()))?
        .after(after_sequence))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn native_logger_keeps_actionable_levels_without_dependency_trace_volume() {
        use log::Log;
        let allowed = |level, target| {
            let metadata = log::Metadata::builder().level(level).target(target).build();
            NATIVE_LOGGER.enabled(&metadata)
        };
        assert!(allowed(log::Level::Error, "jni::wrapper::jnienv"));
        assert!(allowed(log::Level::Warn, "jni::wrapper::jnienv"));
        assert!(allowed(log::Level::Info, MICROPHONE_LOG_TARGET));
        assert!(!allowed(log::Level::Info, "jni::wrapper::jnienv"));
        assert!(!allowed(log::Level::Debug, MICROPHONE_LOG_TARGET));
        assert!(!allowed(log::Level::Trace, "jni::wrapper::jnienv"));
    }

    fn event() -> FrontendDiagnostic {
        FrontendDiagnostic {
            diagnostics: None,
            context: DiagnosticContext::Speech,
            code: DiagnosticCode::UiFault,
            level: Some(DiagnosticLevel::Error),
            native_code: Some(ErrorCode::Provider),
            fault_id: Some(7),
            command: Some(DiagnosticCommand::MicStart),
            cause: Some(DiagnosticCause::Unknown),
            event_name: Some(DiagnosticEvent::IpcFailed),
            redacted_args: Some(1),
        }
    }
    #[test]
    fn only_structured_allowlisted_fields_cross_the_bridge() {
        let valid = serde_json::to_value(event()).unwrap();
        assert!(serde_json::from_value::<FrontendDiagnostic>(valid.clone()).is_ok());
        for field in ["message", "stack", "url", "transcript", "apiKey"] {
            let mut input = valid.clone();
            input[field] = serde_json::json!("private fixture");
            assert!(serde_json::from_value::<FrontendDiagnostic>(input).is_err());
        }
        for field in [
            "context",
            "code",
            "nativeCode",
            "level",
            "command",
            "cause",
            "eventName",
        ] {
            let mut input = valid.clone();
            input[field] = serde_json::json!("private fixture");
            assert!(serde_json::from_value::<FrontendDiagnostic>(input).is_err());
        }
        let mut buffer = Buffer::new();
        let line = serde_json::to_string(&buffer.push(event(), 123).unwrap()).unwrap();
        assert!(line.contains("\"recordedAtMs\":123"));
        assert!(line.contains("\"nativeCode\":\"provider\""));
        assert!(!line.contains("private fixture"));
    }
    #[test]
    fn repeated_native_launches_share_a_root_without_colliding_or_losing_logs() {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path().canonicalize().unwrap();
        let mut first = FileSink::open_run(&root).unwrap();
        first
            .append("native", &serde_json::json!({"code":"first_launch"}))
            .unwrap();
        let first_directory = first.directory.clone();
        drop(first);
        let mut second = FileSink::open_run(&root).unwrap();
        second
            .append("native", &serde_json::json!({"code":"second_launch"}))
            .unwrap();
        assert_ne!(first_directory, second.directory);
        assert!(
            std::fs::read_to_string(first_directory.join("native.jsonl"))
                .unwrap()
                .contains("first_launch")
        );
        assert!(
            std::fs::read_to_string(second.directory.join("native.jsonl"))
                .unwrap()
                .contains("second_launch")
        );
        let exported = archive::save(&root, &root).unwrap();
        let zip = zip::ZipArchive::new(File::open(exported).unwrap()).unwrap();
        assert_eq!(zip.len(), 7); // Export manifest plus three files per launch.
        // Exclusive creation still protects existing files; never truncate/reopen.
        assert!(FileSink::open(&first_directory).is_err());
    }

    #[test]
    fn frontend_errors_survive_sanitization_and_durable_storage() {
        let temp = tempfile::tempdir().unwrap();
        let directory = temp.path().canonicalize().unwrap().join("run");
        let mut sink = FileSink::open(&directory).unwrap();
        let mut value = event();
        value.diagnostics = Some(serde_json::json!({
            "message":"Maximum update depth exceeded; password=private-secret",
            "stack":"at LearningPicker (src/LanguagePickers.tsx:51:9)",
            "metadata":{"request_id":"req-123","transcript":"private sentence"}
        }));
        sanitize_frontend(&mut value);
        sink.append("frontend", &serde_json::to_value(value).unwrap())
            .unwrap();
        let text = std::fs::read_to_string(directory.join("diagnostics.jsonl")).unwrap();
        assert!(text.contains("Maximum update depth exceeded"));
        assert!(text.contains("LanguagePickers.tsx:51:9"));
        assert!(text.contains("req-123"));
        assert!(!text.contains("private-secret"));
        assert!(!text.contains("private sentence"));
    }
    #[test]
    fn progression_commands_cross_the_diagnostic_bridge() {
        let temp = tempfile::tempdir().unwrap();
        let directory = temp.path().canonicalize().unwrap().join("run");
        let mut sink = FileSink::open(&directory).unwrap();
        for command in [
            "get_skill_evidence",
            "get_practice_overview",
            "save_skill_profile",
        ] {
            let mut input = serde_json::to_value(event()).unwrap();
            input["command"] = serde_json::json!(command);
            let decoded: FrontendDiagnostic = serde_json::from_value(input).unwrap();
            let value = serde_json::to_value(decoded).unwrap();
            assert_eq!(value["command"], command);
            sink.append("frontend", &value).unwrap();
            let text = std::fs::read_to_string(directory.join("diagnostics.jsonl")).unwrap();
            let last: serde_json::Value =
                serde_json::from_str(text.lines().last().unwrap()).unwrap();
            assert_eq!(last["event"]["command"], command);
        }
    }
    #[test]
    fn bounded_history_keeps_order_and_reads_have_no_side_effects() {
        let mut buffer = Buffer::new();
        for index in 0..300 {
            buffer.push(event(), index).unwrap();
        }
        assert_eq!(buffer.records.len(), CAPACITY);
        assert_eq!(buffer.after(None).first().unwrap().sequence, 45);
        for _ in 0..20 {
            assert_eq!(
                buffer
                    .after(Some(298))
                    .iter()
                    .map(|r| r.sequence)
                    .collect::<Vec<_>>(),
                vec![299, 300]
            );
        }
        assert_eq!(buffer.sequence, 300);
    }
    #[test]
    fn durable_history_exceeds_ring_and_is_immediately_readable() {
        let temp = tempfile::tempdir().unwrap();
        let directory = temp.path().canonicalize().unwrap().join("run");
        let mut sink = FileSink::open(&directory).unwrap();
        let mut ring = Buffer::new();
        for index in 0..600 {
            let record = ring.push(event(), index).unwrap();
            sink.append("frontend", &serde_json::to_value(record).unwrap())
                .unwrap();
        }
        let bytes = std::fs::read_to_string(directory.join("diagnostics.jsonl")).unwrap();
        let lines: Vec<serde_json::Value> = bytes
            .lines()
            .map(|s| serde_json::from_str(s).unwrap())
            .collect();
        assert_eq!(lines.len(), 600);
        assert_eq!(lines[0]["event"]["sequence"], 1);
        assert_eq!(lines[599]["event"]["sequence"], 600);
        assert_eq!(ring.records.len(), CAPACITY);
        assert!(FileSink::open(&directory).is_err());
        assert_eq!(
            std::fs::read_to_string(directory.join("diagnostics.jsonl")).unwrap(),
            bytes
        );
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            assert_eq!(
                std::fs::metadata(&directory).unwrap().permissions().mode() & 0o777,
                0o700
            );
            assert_eq!(
                std::fs::metadata(directory.join("diagnostics.jsonl"))
                    .unwrap()
                    .permissions()
                    .mode()
                    & 0o777,
                0o600
            );
        }
    }
    #[test]
    #[cfg(unix)]
    fn symlink_and_disk_write_failures_are_explicit() {
        let temp = tempfile::tempdir().unwrap();
        let directory = temp.path().canonicalize().unwrap();
        let link = directory.join("link");
        std::os::unix::fs::symlink(&directory, &link).unwrap();
        assert!(FileSink::open(&link.join("run")).is_err());
        let mut sink = FileSink::open(&directory.join("run")).unwrap();
        // A read-only descriptor deterministically simulates failed disk writes.
        sink.frontend = File::open(directory.join("run/diagnostics.jsonl")).unwrap();
        let error = sink
            .append("frontend", &serde_json::json!({"code":"ui_event"}))
            .unwrap_err();
        let details = error.diagnostics.unwrap();
        assert_eq!(details["stage"], "diagnostic_append");
        assert!(details["os_code"].is_number());
        assert!(details["reason"].is_string());
        assert!(!details.to_string().contains(directory.to_str().unwrap()));
    }
}
