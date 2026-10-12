//! Explicit development-only transport recording. No graph semantics live here.
use crate::model::{AppError, ErrorCode, Result};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::{
    collections::{BTreeMap, VecDeque},
    path::{Path, PathBuf},
    sync::{Mutex, OnceLock},
    time::Instant,
};

pub(super) mod audio;
mod cancellation;
mod capture_state;
mod identity;
mod originals;
pub(crate) use identity::conversation_identity;
pub(super) mod text;
pub(crate) mod unary;

const LIMIT: usize = 64 * 1024 * 1024;
const MAX_CALLS: usize = 10_000;
const MAX_FRAMES: usize = 100_000;
#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum Mode {
    Capture,
    Replay,
}
#[derive(Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
enum Kind {
    Text,
    Protocol,
    Speech,
    Transcription,
    Microphone,
    Identity,
}
#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Frame {
    elapsed_micros: u64,
    payload: Value,
}
#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Record {
    kind: Kind,
    fingerprint: String,
    start_micros: u64,
    frames: Vec<Frame>,
    complete: bool,
    cancelled_micros: Option<u64>,
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Tape {
    format: u32,
    records: Vec<Record>,
}
struct ActiveCall {
    started: Instant,
    record: Option<usize>,
}
struct State {
    directory: PathBuf,
    mode: Mode,
    started: bool,
    used: bool,
    failed: bool,
    failure_reason: Option<&'static str>,
    next_call: u64,
    bytes: usize,
    active: BTreeMap<u64, ActiveCall>,
    records: Vec<Record>,
    replay: BTreeMap<String, VecDeque<Record>>,
    epoch: Instant,
}
static STATE: OnceLock<Mutex<Option<State>>> = OnceLock::new();
fn state() -> &'static Mutex<Option<State>> {
    STATE.get_or_init(|| Mutex::new(None))
}
pub(super) fn error() -> AppError {
    AppError::new(
        ErrorCode::Validation,
        "Workflow recording or replay is incomplete, mismatched, or unavailable. No live fallback was made.",
    )
}
pub(super) fn without_secret(value: Value, key: &str) -> Value {
    match value {
        Value::String(value) if !key.is_empty() => {
            Value::String(value.replace(key, "[credential]"))
        }
        Value::Array(values) => {
            Value::Array(values.into_iter().map(|v| without_secret(v, key)).collect())
        }
        Value::Object(values) => Value::Object(
            values
                .into_iter()
                .map(|(k, v)| (k, without_secret(v, key)))
                .collect(),
        ),
        value => value,
    }
}
fn locked() -> Result<std::sync::MutexGuard<'static, Option<State>>> {
    state().lock().map_err(|_| error())
}
pub(crate) fn is_replay() -> bool {
    state()
        .lock()
        .map(|s| s.as_ref().is_some_and(|s| s.mode == Mode::Replay))
        .unwrap_or(true)
}

/// Every live transport entry must pass this guard. Unsupported network work
/// invalidates replay even if its caller handles the returned error.
pub(crate) fn require_live_network() -> Result<()> {
    let mut state = locked()?;
    if let Some(state) = state.as_mut() {
        state.require_live_network()?;
    }
    Ok(())
}

pub(crate) fn clock_micros() -> u64 {
    state()
        .lock()
        .ok()
        .and_then(|state| {
            state
                .as_ref()
                .map(|state| state.epoch.elapsed().as_micros().min(u64::MAX as u128) as u64)
        })
        .unwrap_or(0)
}

pub(crate) fn configure(directory: &Path, mode: Mode) -> Result<()> {
    let metadata = std::fs::symlink_metadata(directory).map_err(|_| error())?;
    if !directory.is_absolute() || !metadata.is_dir() || metadata.file_type().is_symlink() {
        return Err(error());
    }
    let tape = if mode == Mode::Replay {
        let path = directory.join("native-provider.json");
        let meta = std::fs::symlink_metadata(&path).map_err(|_| error())?;
        if !meta.is_file() || meta.file_type().is_symlink() || meta.len() > LIMIT as u64 {
            return Err(error());
        }
        use std::io::Read;
        let mut bytes = Vec::new();
        std::fs::File::open(path)
            .map_err(|_| error())?
            .take(LIMIT as u64 + 1)
            .read_to_end(&mut bytes)
            .map_err(|_| error())?;
        if bytes.len() > LIMIT {
            return Err(error());
        }
        serde_json::from_slice(&bytes).map_err(|_| error())?
    } else {
        Tape {
            format: 1,
            records: Vec::new(),
        }
    };
    let next = State::new(directory, mode, tape)?;
    let mut state = locked()?;
    if state.is_some() {
        return Err(error());
    }
    *state = Some(next);
    Ok(())
}
pub(crate) fn start() -> Result<()> {
    locked()?.as_mut().ok_or_else(error)?.start()
}
pub(crate) fn stop() -> Result<()> {
    let (directory, tape) = {
        let mut state = locked()?;
        let state = state.as_mut().ok_or_else(error)?;
        (state.directory.clone(), state.stop()?)
    };
    if let Some(tape) = tape {
        let bytes = serde_json::to_vec(&tape)?;
        if bytes.len() > LIMIT {
            return Err(error());
        }
        use std::io::Write;
        let mut file = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(directory.join("native-provider.json"))
            .map_err(|_| error())?;
        file.write_all(&bytes).map_err(|_| error())?;
        file.sync_all().map_err(|_| error())?;
    }
    Ok(())
}

pub(crate) struct Call {
    id: u64,
    recording: bool,
    replay: Option<Record>,
    done: bool,
    cancellation: std::sync::atomic::AtomicBool,
}
pub(super) fn begin(kind: &str, semantic: Value) -> Result<Option<Call>> {
    let kind = match kind {
        "text" => Kind::Text,
        "protocol" => Kind::Protocol,
        "speech" => Kind::Speech,
        "transcription" => Kind::Transcription,
        "microphone" => Kind::Microphone,
        _ => return Err(error()),
    };
    let mut state = locked()?;
    let Some(state) = state.as_mut() else {
        return Ok(None);
    };
    let bytes = serde_json::to_vec(&(kind, semantic))?;
    let fingerprint = format!("{:x}", Sha256::digest(bytes));
    let (recording, replay, id) = state.begin(kind, fingerprint)?;
    Ok(Some(Call {
        id,
        recording,
        replay,
        done: false,
        cancellation: std::sync::atomic::AtomicBool::new(false),
    }))
}
impl Call {
    pub(crate) fn invalidate(&self) {
        if let Ok(mut state) = locked()
            && let Some(state) = state.as_mut()
        {
            state.fail("microphone_capture_invalidated");
        }
    }
    pub(super) fn cancelled(&self) -> Option<u64> {
        self.replay.as_ref().and_then(|r| r.cancelled_micros)
    }
    pub(super) async fn wait(&self, previous: &mut u64, next: u64) -> Result<()> {
        let mut window = cancellation::Window::new(&self.cancellation, self.cancelled().is_some());
        let result = delay(previous, next).await;
        window.complete();
        result
    }
    pub(super) async fn require_cancellation<T>(&self, previous: &mut u64) -> Result<T> {
        let until = self
            .cancelled()
            .ok_or_else(error)?
            .saturating_add(5_000_000);
        self.wait(previous, until.max(*previous)).await?;
        Err(error())
    }
    pub(crate) fn recorded(&self) -> Option<Vec<(u64, Value)>> {
        self.replay.as_ref().map(|r| {
            r.frames
                .iter()
                .map(|f| (f.elapsed_micros, f.payload.clone()))
                .collect()
        })
    }
    pub(crate) fn push(&self, payload: Value) -> Result<()> {
        if !self.recording || self.replay.is_some() {
            return Ok(());
        }
        let mut state = locked()?;
        let state = state.as_mut().ok_or_else(error)?;
        state.push(self.id, payload)
    }
    pub(crate) fn finish(mut self) -> Result<()> {
        let mut state = locked()?;
        let state = state.as_mut().ok_or_else(error)?;
        self.done = true;
        state.finish(self.id)
    }
}
pub(crate) fn microphone_call() -> Result<Option<Call>> {
    begin(
        "microphone",
        serde_json::json!({"channels":1,"format":"f32le"}),
    )
}
impl Drop for Call {
    fn drop(&mut self) {
        if !self.done
            && let Ok(mut state) = state().lock()
            && let Some(state) = state.as_mut()
        {
            state.cancel(
                self.id,
                self.recording && self.replay.is_none(),
                self.cancelled().is_some()
                    && self.cancellation.load(std::sync::atomic::Ordering::Relaxed),
            );
        }
    }
}
pub(super) async fn delay(previous: &mut u64, next: u64) -> Result<()> {
    let delta = next.checked_sub(*previous).ok_or_else(error)?;
    if delta > 600_000_000 {
        return Err(error());
    }
    tokio::time::sleep(std::time::Duration::from_micros(delta)).await;
    *previous = next;
    Ok(())
}

#[cfg(test)]
mod tests;
