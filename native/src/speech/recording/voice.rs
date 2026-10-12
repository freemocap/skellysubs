use crate::ai::connections::access;
use crate::application::Application;
use crate::model::*;
use crate::speech::analysis::spectrogram::LiveSpectrogram;
#[cfg(desktop)]
use crate::speech::recording::audio;
use crate::speech::recording::owner::RecordingOwner;
use std::sync::{Arc, Mutex, MutexGuard};

/// The complete retained recognition result, shared by recording consumers.
#[tauri::command]
pub fn get_transcription_result(
    state: tauri::State<'_, Arc<Application>>,
    recording_id: String,
) -> Result<Option<crate::ai::audio::TranscriptionResult>> {
    crate::speech::recording::results::result(&state.lock()?.connection, &recording_id)
}

pub struct Recording {
    id: String,
    install: String,
    owner: RecordingOwner,
    visit: Option<String>,
    target: access::ResolvedTarget,
    language: crate::ai::audio::TranscriptionLanguage,
    context: Option<String>,
    #[cfg(desktop)]
    capture: audio::Capture,
    /// This recording's live spectrogram. A listening run keeps its own
    /// analysis in its session and leaves this one empty.
    live: Arc<Mutex<super::live_view::LiveView>>,
}
#[derive(Clone)]
pub(super) struct Transcription {
    pub id: String,
    install: String,
    owner: RecordingOwner,
    visit: Option<String>,
    target: access::ResolvedTarget,
    language: crate::ai::audio::TranscriptionLanguage,
    context: Option<String>,
}
impl Transcription {
    pub(super) fn capture_permitted(&self, state: &Application) -> Result<()> {
        let store = state.lock()?;
        if store.learner_id()? != self.install {
            return Err(AppError::new(
                ErrorCode::SessionExpired,
                "Recording belongs to a different workspace.",
            ));
        }
        super::transcription::permitted(&store.connection, &self.owner, &self.target)?;
        if crate::ai::connections::configuration::config(&store.connection)?.paused {
            return Err(AppError::new(
                ErrorCode::AdmissionHeld,
                "Listening stopped: AI execution is paused.",
            ));
        }
        if let RecordingOwner::DrillItem(item) = &self.owner {
            let current = crate::drill::sessions::active_visit(&store.connection, item)?;
            if Some(current.as_str()) != self.visit.as_deref() {
                return Err(AppError::new(
                    ErrorCode::Conflict,
                    "Listening stopped because the practice visit changed.",
                ));
            }
        }
        Ok(())
    }
}
impl Recording {
    pub(super) fn id(&self) -> &str {
        &self.id
    }
    pub(super) fn request(&self) -> Transcription {
        Transcription {
            id: self.id.clone(),
            install: self.install.clone(),
            owner: self.owner.clone(),
            visit: self.visit.clone(),
            target: self.target.clone(),
            language: self.language.clone(),
            context: self.context.clone(),
        }
    }
    #[cfg(desktop)]
    pub(super) fn drain(&self) -> Result<(u32, Vec<f32>)> {
        self.capture.drain().map_err(fault)
    }
}
fn fault(message: impl Into<String>) -> AppError {
    AppError::new(ErrorCode::Provider, message)
}
#[tauri::command]
pub async fn mic_start(
    state: tauri::State<'_, Arc<Application>>,
    owner: RecordingOwner,
) -> Result<RecordingStarted> {
    let state = state.inner().clone();
    let prepared = super::preflight::prepare(&state, &owner).await?;
    tauri::async_runtime::spawn_blocking(move || {
        start_prepared_capture(&state, owner, Some(prepared))
    })
    .await
    .map_err(|cause| {
        crate::diagnostics::failures::join(
            &cause,
            "voice.rs",
            fault("Microphone startup stopped unexpectedly."),
        )
    })?
}
/// One capture at a time, whatever owns it: Chat and Drill share this slot.
#[cfg(test)]
pub(crate) fn start_capture(
    state: &Arc<Application>,
    owner: RecordingOwner,
) -> Result<RecordingStarted> {
    start_prepared_capture(state, owner, None)
}
pub(super) fn start_prepared_capture(
    state: &Arc<Application>,
    owner: RecordingOwner,
    prepared: Option<super::preflight::Prepared>,
) -> Result<RecordingStarted> {
    let test = state
        .microphone_test
        .lock()
        .map_err(|_| fault("Microphone test state unavailable."))?;
    super::microphone_test::require_idle(&test)?;
    let mut slot = state.capture.lock().map_err(|_| {
        crate::diagnostics::failures::poisoned(fault("Microphone state unavailable."))
    })?;
    if slot.is_some() {
        return Err(fault("A recording is already running."));
    }
    let store = state.lock()?;
    // Language, variety and recognizer context come from the owner's record.
    let scope = owner.scope(&store)?;
    let target = if let Some(prepared) = prepared {
        prepared.validate(&store.connection, &scope.language_context)?;
        prepared.target
    } else {
        crate::ai::connections::speech_routing::resolve(
            &store.connection,
            access::Capability::Transcription,
            &scope.language_context,
        )?
    };
    let visit = match &owner {
        RecordingOwner::Conversation(_) => None,
        RecordingOwner::DrillItem(item) => Some(crate::drill::sessions::active_visit(
            &store.connection,
            item,
        )?),
    };
    crate::ai::audio::validate_transcription_language(&target, &scope.language)?;
    let microphone = super::microphone::selected(&store.connection)?;
    let install = store.learner_id()?;
    drop(store);
    let recording = Recording {
        id: uuid::Uuid::new_v4().to_string(),
        install,
        owner,
        visit,
        target,
        language: scope.language,
        context: scope.context,
        #[cfg(desktop)]
        capture: audio::start(microphone.as_deref()).map_err(fault)?,
        live: Default::default(),
    };
    let started = RecordingStarted {
        #[cfg(desktop)]
        device_label: Some(recording.capture.device_label().to_owned()),
        #[cfg(mobile)]
        device_label: None,
        recording_id: recording.id.clone(),
        browser_capture: cfg!(mobile),
        browser_device_id: if cfg!(mobile) { microphone } else { None },
        #[cfg(desktop)]
        samples_per_second: recording.capture.wave_samples_per_second(),
        #[cfg(mobile)]
        samples_per_second: 750.0,
    };
    // A new recording supersedes a failed take waiting for Retry.
    failed_take(state)?.start(&recording.id);
    *slot = Some(recording);
    Ok(started)
}
#[tauri::command]
pub fn mic_wave(
    state: tauri::State<'_, Arc<Application>>,
    recording_id: String,
) -> Result<Vec<f32>> {
    {
        let slot = state.capture.lock().map_err(|_| {
            crate::diagnostics::failures::poisoned(fault("Microphone state unavailable."))
        })?;
        if let Some(recording) = slot.as_ref().filter(|r| r.id == recording_id) {
            #[cfg(desktop)]
            {
                return recording.capture.take_wave().map_err(fault);
            }
            #[cfg(mobile)]
            {
                let _ = recording;
                return Ok(Vec::new());
            }
        }
    }
    // A listening run releases the microphone before it reports that it has
    // stopped; between the two, its waveform is simply finished. The capture lock
    // is released first: starting a run takes the listening lock, then capture.
    if super::continuous::is_listening_run(&state, &recording_id) {
        return Ok(Vec::new());
    }
    Err(fault("Recording is no longer active."))
}
/// A single recording's live spectrogram after `after_seconds`: the analysis a
/// listening run shows, so Chat draws the same stream as Practice. Desktop
/// capture is analysed as this is polled; browser capture as `mic_push` delivers it.
#[tauri::command]
pub async fn mic_spectrogram(
    state: tauri::State<'_, Arc<Application>>,
    recording_id: String,
    after_seconds: Option<f64>,
) -> Result<Option<LiveSpectrogram>> {
    live_spectrogram(&state, &recording_id, after_seconds)
}
pub(super) fn live_spectrogram(
    state: &Application,
    recording_id: &str,
    after_seconds: Option<f64>,
) -> Result<Option<LiveSpectrogram>> {
    if after_seconds.is_some_and(|n| !n.is_finite() || n < 0.0) {
        return Err(AppError::new(
            ErrorCode::Validation,
            "Invalid spectrum cursor.",
        ));
    }
    // A listening run drains its own samples; its session owns its spectrogram.
    if super::continuous::is_listening_run(state, recording_id) {
        return Err(AppError::new(
            ErrorCode::Conflict,
            "A listening run reports its spectrogram through its session.",
        ));
    }
    let (live, fresh) = {
        let slot = state.capture.lock().map_err(|_| {
            crate::diagnostics::failures::poisoned(fault("Microphone state unavailable."))
        })?;
        let recording = slot
            .as_ref()
            .filter(|r| r.id == recording_id)
            .ok_or_else(|| fault("Recording is no longer active."))?;
        #[cfg(desktop)]
        let fresh = Some(recording.capture.take_unanalysed().map_err(fault)?);
        #[cfg(mobile)]
        let fresh: Option<(u32, Vec<f32>)> = None;
        (recording.live.clone(), fresh)
    };
    let mut view = live
        .lock()
        .map_err(|_| fault("Live spectrum unavailable."))?;
    if let Some((rate, pcm)) = fresh {
        view.feed(rate, &pcm);
    }
    Ok(view.snapshot_since(after_seconds))
}
/// Browser capture sends ordered copies of its PCM for the live spectrogram;
/// the recording itself still arrives whole, as the WAV for `mic_transcribe`.
#[tauri::command]
pub async fn mic_push(
    state: tauri::State<'_, Arc<Application>>,
    recording_id: String,
    sequence: u32,
    sample_rate: u32,
    samples: Vec<f32>,
) -> Result<()> {
    push_live(&state, &recording_id, sequence, sample_rate, samples)
}
pub(super) fn push_live(
    state: &Application,
    recording_id: &str,
    sequence: u32,
    sample_rate: u32,
    samples: Vec<f32>,
) -> Result<()> {
    let live = {
        let slot = state.capture.lock().map_err(|_| {
            crate::diagnostics::failures::poisoned(fault("Microphone state unavailable."))
        })?;
        slot.as_ref()
            .filter(|r| r.id == recording_id)
            .map(|r| r.live.clone())
            .ok_or_else(|| fault("Recording is no longer active."))?
    };
    let mut view = live
        .lock()
        .map_err(|_| fault("Live spectrum unavailable."))?;
    view.push_browser(sequence, sample_rate, samples)
        .map_err(|message| AppError::new(ErrorCode::Validation, message))
}
#[tauri::command]
pub fn mic_cancel(state: tauri::State<'_, Arc<Application>>, recording_id: String) -> Result<()> {
    super::continuous::cancel_session(&state, &recording_id);
    let mut slot = state.capture.lock().map_err(|_| {
        crate::diagnostics::failures::poisoned(fault("Microphone state unavailable."))
    })?;
    if slot.as_ref().is_some_and(|r| r.id == recording_id) {
        slot.take();
    }
    Ok(())
}
#[tauri::command]
pub async fn mic_transcribe(
    state: tauri::State<'_, Arc<Application>>,
    recording_id: String,
    audio_base64: Option<String>,
) -> Result<crate::speech::analysis::audio_inspection::TranscriptionInspectionResult> {
    let recording = {
        let mut slot = state.capture.lock().map_err(|_| {
            crate::diagnostics::failures::poisoned(fault("Microphone state unavailable."))
        })?;
        if slot.as_ref().is_none_or(|r| r.id != recording_id) {
            return Err(fault("Recording is no longer active."));
        }
        slot.take()
            .ok_or_else(|| fault("Recording is unavailable."))?
    };
    let request = recording.request();
    #[cfg(desktop)]
    if audio_base64.is_some() {
        return Err(fault("Desktop capture does not accept browser audio."));
    }
    #[cfg(desktop)]
    let wav = tauri::async_runtime::spawn_blocking(move || recording.capture.finish())
        .await
        .map_err(|cause| {
            crate::diagnostics::failures::join(
                &cause,
                "voice.rs",
                fault("Audio processing stopped unexpectedly."),
            )
        })?
        .map_err(fault)?;
    #[cfg(mobile)]
    let wav = {
        use base64::Engine;
        let encoded = audio_base64.ok_or_else(|| fault("Microphone audio is missing."))?;
        if encoded.len() > 24 * 1024 * 1024 {
            return Err(fault("Recording exceeds its size limit."));
        }
        let bytes = base64::engine::general_purpose::STANDARD
            .decode(encoded)
            .map_err(|cause| {
                crate::diagnostics::failures::base64(
                    &cause,
                    "voice.rs_base64",
                    fault("Invalid recording encoding."),
                )
            })?;
        if bytes.len() < 44 || &bytes[..4] != b"RIFF" || &bytes[8..12] != b"WAVE" {
            return Err(fault("Recording must be WAV audio."));
        }
        bytes
    };
    transcribe_holding_failure(state.inner().clone(), request, wav).await
}

/// A manual take whose transcription failed, held in memory so Retry can send
/// the same audio again. Only the latest failed take is held, until a retry
/// succeeds or a new recording starts; a restart loses it.
struct FailedTake {
    /// The take's own identity, which the learner's Retry names.
    take: String,
    request: Transcription,
    wav: Vec<u8>,
}

#[derive(Default)]
pub(crate) struct RetryTake {
    // Capture selects this before asynchronous work begins. Older completions
    // must not clear newer audio or restore a take superseded by another capture.
    latest: String,
    held: Option<FailedTake>,
}

impl RetryTake {
    fn start(&mut self, id: &str) {
        self.latest = id.to_owned();
        self.held = None;
    }
}

fn failed_take(state: &Application) -> Result<MutexGuard<'_, RetryTake>> {
    state
        .failed_take
        .lock()
        .map_err(|_| crate::diagnostics::failures::poisoned(fault("Microphone state unavailable.")))
}

/// Transcribes a manual take, holding it for Retry if transcription fails.
async fn transcribe_holding_failure(
    state: Arc<Application>,
    request: Transcription,
    wav: Vec<u8>,
) -> Result<crate::speech::analysis::audio_inspection::TranscriptionInspectionResult> {
    let result = transcribe(state.clone(), request.clone(), wav.clone()).await;
    let mut slot = failed_take(&state)?;
    if slot.latest == request.id {
        slot.held = result.is_err().then(|| FailedTake {
            take: request.id.clone(),
            request,
            wav,
        });
    }
    result
}

/// Transcribes a held failed take again, as a new attempt with its own identity;
/// the failed attempt stays recorded. Transcription results are cached by the
/// audio, so provider work that already succeeded is not paid for twice. The
/// take stays held if this attempt fails too.
async fn retry_held_take(
    state: Arc<Application>,
    take: &str,
) -> Result<crate::speech::analysis::audio_inspection::TranscriptionInspectionResult> {
    let held = {
        let mut slot = failed_take(&state)?;
        match slot.held.take() {
            Some(held) if held.take == take => held,
            other => {
                slot.held = other;
                return Err(AppError::new(
                    ErrorCode::NotFound,
                    "This recording's audio is no longer available to send again.",
                ));
            }
        }
    };
    let mut request = held.request.clone();
    request.id = uuid::Uuid::new_v4().to_string();
    let result = transcribe(state.clone(), request, held.wav.clone()).await;
    if result.is_err() {
        let mut slot = failed_take(&state)?;
        if slot.latest == held.take {
            slot.held = Some(held);
        }
    }
    result
}

/// Sends a failed take's audio for transcription again (Retry).
#[tauri::command]
pub async fn mic_retry_transcription(
    state: tauri::State<'_, Arc<Application>>,
    recording_id: String,
) -> Result<crate::speech::analysis::audio_inspection::TranscriptionInspectionResult> {
    retry_held_take(state.inner().clone(), &recording_id).await
}

// Manual clips and segmented utterances share inspection, admission, receipts,
// provider execution, and durable publication without a second attempt writer.
pub(super) async fn transcribe(
    state: Arc<Application>,
    recording: Transcription,
    wav: Vec<u8>,
) -> Result<crate::speech::analysis::audio_inspection::TranscriptionInspectionResult> {
    let recording_id = recording.id.clone();
    let validate = || {
        let store = state.lock()?;
        if store.learner_id()? != recording.install {
            return Err(AppError::new(
                ErrorCode::SessionExpired,
                "Recording belongs to a different workspace.",
            ));
        }
        super::transcription::permitted(&store.connection, &recording.owner, &recording.target)
    };
    validate()?;
    let mut inspection = state
        .inspect_audio(wav.clone(), recording.id.clone(), recording.owner.clone())
        .await?;
    {
        let mut store = state.lock()?;
        if store.learner_id()? != recording.install {
            return Err(AppError::new(
                ErrorCode::SessionExpired,
                "Recording belongs to a different workspace.",
            ));
        }
        store.reserve_transcription_in_visit(
            &recording_id,
            &recording.owner,
            &recording.target,
            recording.visit.as_deref(),
        )?;
    }
    use base64::Engine;
    let audio_base64 = base64::engine::general_purpose::STANDARD.encode(&wav);

    let input = crate::ai::audio::TranscriptionRequest {
        wav,
        language: recording.language.clone(),
        context: recording.context.clone(),
    };
    let result = state
        .shared_transcription(
            recording.target.clone(),
            input.clone(),
            recording.install.clone(),
            &recording_id,
            validate,
        )
        .await
        .and_then(|saved| {
            let result = serde_json::from_slice(&saved.payload).map_err(|_| {
                AppError::new(
                    ErrorCode::Storage,
                    "Saved transcription has an invalid shape.",
                )
            })?;
            let mut diagnostics = saved.metadata["diagnostics"].clone();
            if diagnostics.is_null() {
                diagnostics = serde_json::json!({});
            }
            diagnostics["sourceExecutionId"] = serde_json::json!(saved.execution);
            diagnostics["cacheHit"] = serde_json::json!(saved.cached);
            Ok(crate::ai::audio::TranscriptionOutcome {
                result,
                diagnostics: Some(diagnostics),
            })
        });
    {
        let mut store = state.lock()?;
        if store.learner_id()? != recording.install {
            return Err(AppError::new(
                ErrorCode::SessionExpired,
                "Recording belongs to a different workspace.",
            ));
        }
        if let Err(error) = &result {
            store.note_refusal(&recording.target, error)?;
        }
    }
    let mut diagnostics = result.as_ref().ok().and_then(|r| r.diagnostics.clone());
    if result.is_ok() && matches!(recording.owner, RecordingOwner::DrillItem(_)) {
        let reliability = crate::drill::reliability::assess(&inspection, diagnostics.as_ref());
        diagnostics.get_or_insert_with(|| serde_json::json!({}))["drill_reliability"] =
            serde_json::to_value(reliability)?;
    }
    let result = result.map(|response| {
        crate::speech::analysis::audio_inspection::attach_words(
            &mut inspection,
            response.result.timing.as_ref(),
        );
        response.result
    });
    let evidence = result.as_ref().ok();
    let text = {
        let mut store = state.lock()?;
        if store.learner_id()? != recording.install {
            return Err(AppError::new(
                ErrorCode::SessionExpired,
                "Recording belongs to a different workspace.",
            ));
        }
        store.publish_recording_result(
            &recording_id,
            &recording.owner,
            &recording.target,
            result
                .as_ref()
                .map(|value| value.text.clone())
                .map_err(Clone::clone),
            diagnostics.as_ref(),
            Some(&input.wav),
            evidence,
        )?
    };
    {
        let store = state.lock()?;
        let digest = crate::ai::results::digest(&input.wav);
        crate::speech::analysis::signal_cache::save(
            &store.connection,
            &digest,
            &crate::speech::analysis::audio_inspection::AudioSignal::from_inspection(&inspection),
        )?;
        store.prune_drill_audio()?;
    }
    Ok(
        crate::speech::analysis::audio_inspection::TranscriptionInspectionResult {
            text,
            inspection,
            audio_base64,
            diagnostics,
        },
    )
}

#[cfg(all(test, desktop))]
pub(super) fn fixture(state: &Application, owner: RecordingOwner, pcm: Vec<f32>) -> Recording {
    let store = state.lock().unwrap();
    let scope = owner.scope(&store).unwrap();
    let visit = match &owner {
        RecordingOwner::DrillItem(id) => {
            Some(crate::drill::sessions::active_visit(&store.connection, id).unwrap())
        }
        _ => None,
    };
    Recording {
        id: "continuous-fixture".into(),
        install: store.snapshot().unwrap().learner.id,
        owner,
        visit,
        target: access::resolve(&store.connection, access::Capability::Transcription).unwrap(),
        language: scope.language,
        context: scope.context,
        capture: audio::fixture(pcm, 8000),
        live: Default::default(),
    }
}

#[cfg(all(test, desktop))]
#[path = "transcription_execution_tests.rs"]
mod tests;
