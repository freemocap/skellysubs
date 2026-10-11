//! Microphone capture, in the core.
//!
//! The webview cannot do this on every platform. `navigator.mediaDevices`
//! exists only in a secure context, and WKWebView does not treat Tauri's
//! `tauri://localhost` scheme as one — the only mechanism that would is a
//! private Apple API on `WKProcessPool`. So a packaged macOS build has no
//! browser recording API at all. Rather than keep one platform on a different
//! recorder, every desktop platform records here.
//!
//! cpal opens the input device; hound writes the 16-bit WAV that Whisper is
//! sent. Both are held to one code path: there is no software fallback if the
//! device cannot be opened, and no second recorder to try instead.

use super::wav::encode_wav;
use std::sync::mpsc::{self, Sender};
use std::sync::{Arc, Mutex};

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{FromSample, Sample};
use log::info;

/// Keep every Nth sample for the waveform. The UI draws a scrolling
/// oscilloscope at ~60fps and cannot use 48,000 points a second; one in 64 is
/// about 750/s, which is more than the strip can show.
const WAVE_STRIDE: usize = 64;

/// Cap on the waveform backlog, so a recording nobody is watching cannot grow
/// the buffer without bound.
const WAVE_MAX: usize = 8_192;

/// Longest recording accepted, in seconds. A held button or a stuck auto-stop
/// would otherwise fill memory with audio no one is going to transcribe.
/// Reaching it is an error, not a silent truncation — see [`Capture::finish`].
const MAX_SECONDS: u32 = 120;

#[derive(Default)]
pub(super) struct Buffers {
    /// Every captured sample, mono, for the WAV.
    pcm: Vec<f32>,
    /// How much of `pcm` the live spectrogram has already read.
    analysed: usize,
    total_samples: usize,
    /// Decimated samples the UI has not drawn yet.
    wave: Vec<f32>,
    /// Set when `pcm` hit [`MAX_SECONDS`]. The recording is no longer complete,
    /// so finishing it must fail rather than hand back a truncated clip.
    overflowed: bool,
    error: Option<String>,
}

/// A recording in progress.
///
/// `cpal::Stream` is not `Send`, so it cannot be parked in shared state. It
/// lives on its own thread instead, which builds it, plays it, and blocks until
/// told to stop — the thread IS the stream's lifetime.
pub struct Capture {
    stop: Sender<()>,
    buffers: Arc<Mutex<Buffers>>,
    sample_rate: u32,
    thread: Option<std::thread::JoinHandle<()>>,
    device_label: String,
    #[cfg(feature = "workflow-capture")]
    workflow: Option<super::workflow::Session>,
}

fn open(name: Option<&str>) -> Result<cpal::Device, String> {
    let host = cpal::default_host();
    match name {
        Some(wanted) => host
            .input_devices()
            .map_err(|e| format!("The system would not list microphones: {e}"))?
            .find(|d| d.name().map(|n| n == wanted).unwrap_or(false))
            .ok_or_else(|| {
                format!(
                    "The microphone \"{wanted}\" is not connected. Pick another one in \
                     Settings, or choose System default."
                )
            }),
        None => host
            .default_input_device()
            .ok_or_else(|| "This computer has no microphone available.".to_string()),
    }
}

/// Mix a frame down to mono and record it.
pub(super) fn push(buffers: &Arc<Mutex<Buffers>>, mono: &[f32], limit: usize) {
    let mut b = buffers.lock().expect("audio buffers");
    if b.overflowed {
        return;
    }
    if b.pcm.len() + mono.len() > limit {
        b.overflowed = true;
        return;
    }
    let offset = b.total_samples;
    b.total_samples += mono.len();
    b.pcm.extend_from_slice(mono);
    // Strided against the running total, not the callback, so the decimation
    // stays even across buffer boundaries.
    let mut i = offset.next_multiple_of(WAVE_STRIDE);
    while i < offset + mono.len() {
        b.wave.push(mono[i - offset]);
        i += WAVE_STRIDE;
    }
    if b.wave.len() > WAVE_MAX {
        let excess = b.wave.len() - WAVE_MAX;
        b.wave.drain(..excess);
    }
}

/// Every input the host lists, with the format [`start`] would open it in. The
/// id is the name, because [`open`] finds devices by name.
pub(super) fn input_devices() -> Result<Vec<super::microphone::MicrophoneDevice>, String> {
    let host = cpal::default_host();
    let default = host.default_input_device().and_then(|d| d.name().ok());
    let mut listed = Vec::new();
    for device in host
        .input_devices()
        .map_err(|e| format!("The system would not list microphones: {e}"))?
    {
        let name = device
            .name()
            .map_err(|e| format!("Could not identify a microphone: {e}"))?;
        let (channels, sample_rate, unavailable) = match device.default_input_config() {
            Ok(config) => (Some(config.channels()), Some(config.sample_rate().0), None),
            Err(e) => (None, None, Some(e.to_string())),
        };
        listed.push(super::microphone::MicrophoneDevice {
            is_default: default.as_deref() == Some(name.as_str()),
            id: name.clone(),
            label: name,
            channels,
            sample_rate,
            unavailable,
        });
    }
    Ok(listed)
}

/// Build the input stream for one sample format.
fn stream_for<T>(
    device: &cpal::Device,
    config: &cpal::StreamConfig,
    buffers: Arc<Mutex<Buffers>>,
    limit: usize,
    #[cfg(feature = "workflow-capture")] sink: Option<super::workflow::Sink>,
) -> Result<cpal::Stream, cpal::BuildStreamError>
where
    T: cpal::SizedSample + 'static,
    f32: FromSample<T>,
{
    let channels = config.channels as usize;
    let errors = buffers.clone();
    device.build_input_stream(
        config,
        move |data: &[T], _: &cpal::InputCallbackInfo| {
            let mono: Vec<f32> = data
                .chunks(channels)
                .map(|frame| {
                    frame.iter().map(|s| f32::from_sample(*s)).sum::<f32>() / channels as f32
                })
                .collect();
            push(&buffers, &mono, limit);
            #[cfg(feature = "workflow-capture")]
            if let Some(sink) = &sink
                && let Err(error) = sink.push(&mono)
            {
                buffers.lock().expect("audio buffers").error = Some(error);
            }
        },
        move |e| {
            errors.lock().expect("audio buffers").error =
                Some(format!("Microphone stream failed: {e}"));
        },
        None,
    )
}

/// Open the device and start recording.
///
/// Blocks until the stream is actually playing, so a device that cannot be
/// opened fails here — at the moment the user pressed the button — rather than
/// producing an empty recording later.
pub fn start(device_name: Option<&str>) -> Result<Capture, String> {
    #[cfg(feature = "workflow-capture")]
    {
        super::workflow::start(device_name)
    }
    #[cfg(not(feature = "workflow-capture"))]
    {
        start_live(device_name)
    }
}

pub(super) fn start_live(
    device_name: Option<&str>,
    #[cfg(feature = "workflow-capture")] sink: Option<super::workflow::Sink>,
) -> Result<Capture, String> {
    let buffers = Arc::new(Mutex::new(Buffers::default()));
    let (stop_tx, stop_rx) = mpsc::channel::<()>();
    let (ready_tx, ready_rx) = mpsc::channel::<Result<(u32, String), String>>();

    let thread_buffers = buffers.clone();
    let wanted = device_name.map(str::to_string);
    let thread = std::thread::spawn(move || {
        let built = (|| -> Result<(cpal::Stream, u32, String), String> {
            let device = open(wanted.as_deref())?;
            let label = device
                .name()
                .map_err(|e| format!("Could not identify microphone: {e}"))?;
            let supported = device
                .default_input_config()
                .map_err(|e| format!("The microphone \"{label}\" would not open: {e}"))?;
            let sample_rate = supported.sample_rate().0;
            let format = supported.sample_format();
            let config: cpal::StreamConfig = supported.into();
            let limit = (sample_rate * MAX_SECONDS) as usize;
            let stream = match format {
                cpal::SampleFormat::F32 => stream_for::<f32>(
                    &device,
                    &config,
                    thread_buffers,
                    limit,
                    #[cfg(feature = "workflow-capture")]
                    sink,
                ),
                cpal::SampleFormat::I16 => stream_for::<i16>(
                    &device,
                    &config,
                    thread_buffers,
                    limit,
                    #[cfg(feature = "workflow-capture")]
                    sink,
                ),
                cpal::SampleFormat::U16 => stream_for::<u16>(
                    &device,
                    &config,
                    thread_buffers,
                    limit,
                    #[cfg(feature = "workflow-capture")]
                    sink,
                ),
                other => {
                    return Err(format!(
                        "The microphone \"{label}\" records in a format this app cannot \
                         read ({other}). Pick another one in Settings."
                    ));
                }
            }
            .map_err(|e| format!("The microphone \"{label}\" would not start: {e}"))?;
            stream
                .play()
                .map_err(|e| format!("The microphone \"{label}\" would not start: {e}"))?;
            Ok((stream, sample_rate, label))
        })();

        match built {
            Ok((stream, sample_rate, label)) => {
                if ready_tx.send(Ok((sample_rate, label))).is_err() {
                    return; // caller gave up; drop the stream
                }
                // The stream stops when it is dropped, so this thread parks
                // here holding it until `finish` or `cancel` says otherwise.
                let _ = stop_rx.recv();
                drop(stream);
            }
            Err(e) => {
                let _ = ready_tx.send(Err(e));
            }
        }
    });

    match ready_rx.recv() {
        Ok(Ok((sample_rate, device_label))) => {
            info!("[mic] capture started at {sample_rate}Hz");
            Ok(Capture {
                stop: stop_tx,
                buffers,
                sample_rate,
                thread: Some(thread),
                device_label,
                #[cfg(feature = "workflow-capture")]
                workflow: None,
            })
        }
        Ok(Err(e)) => Err(e),
        Err(_) => Err("The recording thread stopped before it could start.".into()),
    }
}

impl Capture {
    #[cfg(feature = "workflow-capture")]
    pub(super) fn attach_workflow(&mut self, session: super::workflow::Session) {
        self.workflow = Some(session);
    }
    #[cfg(feature = "workflow-capture")]
    pub(super) fn sample_rate(&self) -> u32 {
        self.sample_rate
    }
    pub(super) fn device_label(&self) -> &str {
        &self.device_label
    }

    /// Samples the UI has not drawn yet, removed from the buffer as they are
    /// handed over.
    pub fn wave_samples_per_second(&self) -> f64 {
        self.sample_rate as f64 / WAVE_STRIDE as f64
    }

    pub fn take_wave(&self) -> Result<Vec<f32>, String> {
        let mut b = self.buffers.lock().expect("audio buffers");
        if let Some(error) = &b.error {
            return Err(error.clone());
        }
        if b.overflowed {
            return Err(format!(
                "Recording exceeded {MAX_SECONDS} seconds. Record a shorter message."
            ));
        }
        Ok(std::mem::take(&mut b.wave))
    }

    /// Bounded worker consumes PCM; the device callback only appends samples.
    pub(super) fn drain(&self) -> Result<(u32, Vec<f32>), String> {
        let mut b = self
            .buffers
            .lock()
            .map_err(|_| "Audio buffers unavailable.")?;
        if let Some(error) = &b.error {
            return Err(error.clone());
        }
        if b.overflowed {
            return Err("Microphone buffer overflow; recording stopped.".into());
        }
        b.analysed = 0;
        Ok((self.sample_rate, std::mem::take(&mut b.pcm)))
    }

    /// Samples captured since the last call, copied for the live spectrogram.
    /// The recording keeps every sample for its WAV.
    pub(super) fn take_unanalysed(&self) -> Result<(u32, Vec<f32>), String> {
        let mut b = self
            .buffers
            .lock()
            .map_err(|_| "Audio buffers unavailable.")?;
        if let Some(error) = &b.error {
            return Err(error.clone());
        }
        if b.overflowed {
            return Err(format!(
                "Recording exceeded {MAX_SECONDS} seconds. Record a shorter message."
            ));
        }
        let start = b.analysed.min(b.pcm.len());
        let fresh = b.pcm[start..].to_vec();
        b.analysed = b.pcm.len();
        Ok((self.sample_rate, fresh))
    }

    fn halt(&mut self) {
        let _ = self.stop.send(());
        if let Some(t) = self.thread.take()
            && t.join().is_err()
        {
            self.buffers.lock().expect("audio buffers").error =
                Some("Recording thread failed.".into());
        }
    }

    /// Stop recording and return the WAV.
    pub fn finish(mut self) -> Result<Vec<u8>, String> {
        self.halt();
        let result = self.finished_wav();
        #[cfg(feature = "workflow-capture")]
        if let Some(session) = self.workflow.take() {
            session.finish("finish", result.as_ref().err().cloned())?;
        }
        result
    }
    fn finished_wav(&self) -> Result<Vec<u8>, String> {
        let b = self.buffers.lock().expect("audio buffers");
        if let Some(error) = &b.error {
            return Err(error.clone());
        }
        if b.overflowed {
            return Err(format!(
                "That recording passed {MAX_SECONDS} seconds and was not kept. \
                 Record a shorter one."
            ));
        }
        if b.pcm.is_empty() {
            return Err(format!(
                "No audio came from \"{}\". Check that it is not muted, and that \
                 SkellySpeak is allowed to use the microphone in your system settings.",
                self.device_label
            ));
        }
        let wav = encode_wav(&b.pcm, self.sample_rate)?;
        info!(
            "[mic] capture finished: {} samples, {} bytes of WAV",
            b.pcm.len(),
            wav.len()
        );
        Ok(wav)
    }
}

impl Drop for Capture {
    fn drop(&mut self) {
        self.halt();
        #[cfg(feature = "workflow-capture")]
        if let Some(session) = self.workflow.take() {
            let _ = session.finish("cancel", None);
        }
    }
}

#[cfg(test)]
pub(super) fn fixture(samples: Vec<f32>, sample_rate: u32) -> Capture {
    let (stop, _) = mpsc::channel();
    Capture {
        stop,
        buffers: Arc::new(Mutex::new(Buffers {
            pcm: samples,
            ..Buffers::default()
        })),
        sample_rate,
        thread: None,
        device_label: "fixture".into(),
        #[cfg(feature = "workflow-capture")]
        workflow: None,
    }
}

#[cfg(feature = "workflow-capture")]
pub(super) fn replay_capture(
    rate: u32,
    drive: impl FnOnce(Arc<Mutex<Buffers>>, mpsc::Receiver<()>) + Send + 'static,
) -> Capture {
    let buffers = Arc::new(Mutex::new(Buffers::default()));
    let (stop, receive) = mpsc::channel();
    let target = buffers.clone();
    let thread = std::thread::spawn(move || drive(target, receive));
    Capture {
        stop,
        buffers,
        sample_rate: rate,
        thread: Some(thread),
        device_label: "Recorded microphone".into(),
        workflow: None,
    }
}
#[cfg(feature = "workflow-capture")]
pub(super) fn replay_error(buffers: &Arc<Mutex<Buffers>>, error: String) {
    buffers.lock().unwrap_or_else(|e| e.into_inner()).error = Some(error);
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn wav_contains_mono_clamped_pcm_with_actual_rate() {
        let bytes = encode_wav(&[-2.0, 0.0, 2.0], 48000).unwrap();
        let mut reader = hound::WavReader::new(std::io::Cursor::new(bytes)).unwrap();
        assert_eq!(reader.spec().channels, 1);
        assert_eq!(reader.spec().sample_rate, 48000);
        assert_eq!(
            reader
                .samples::<i16>()
                .collect::<Result<Vec<_>, _>>()
                .unwrap(),
            vec![-32767, 0, 32767]
        );
    }
    #[test]
    fn live_analysis_reads_each_sample_once_and_the_wav_keeps_them_all() {
        let capture = fixture(vec![0.1, 0.2, 0.3], 8000);
        assert_eq!(
            capture.take_unanalysed().unwrap(),
            (8000, vec![0.1, 0.2, 0.3])
        );
        assert_eq!(capture.take_unanalysed().unwrap(), (8000, vec![]));
        capture.buffers.lock().unwrap().pcm.push(0.4);
        assert_eq!(capture.take_unanalysed().unwrap(), (8000, vec![0.4]));
        let wav = capture.finish().unwrap();
        let reader = hound::WavReader::new(std::io::Cursor::new(wav)).unwrap();
        assert_eq!(reader.len(), 4);
    }
    #[test]
    fn recording_bound_is_explicit_and_does_not_silently_truncate() {
        let buffers = Arc::new(Mutex::new(Buffers::default()));
        push(&buffers, &[0.0, 0.1], 2);
        push(&buffers, &[0.2], 2);
        let buffer = buffers.lock().unwrap();
        assert!(buffer.overflowed);
        assert_eq!(buffer.pcm.len(), 2);
    }
}
