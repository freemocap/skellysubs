//! Development microphone capture at the mono PCM boundary. Replay uses the
//! ordinary buffers, waveform, segmentation and WAV encoder without a device.
use super::audio::{self, Capture};
use crate::ai::transport::workflow::{self, Call};
use base64::{Engine, engine::general_purpose::STANDARD};
use serde::{Deserialize, Serialize};
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
    mpsc::{self, SyncSender},
};
use std::time::{Duration, Instant};

fn failure() -> String {
    "Workflow microphone capture or replay is incomplete or mismatched.".into()
}
#[derive(Serialize, Deserialize)]
#[serde(tag = "type", deny_unknown_fields)]
enum Frame {
    Opened {
        result: Result<u32, String>,
    },
    Samples {
        micros: u64,
        pcm: String,
    },
    End {
        action: String,
        error: Option<String>,
    },
}
enum Message {
    Samples(u64, Vec<f32>),
    End(String, Option<String>),
}
#[derive(Clone)]
pub(super) struct Sink {
    sender: SyncSender<Message>,
    failed: Arc<AtomicBool>,
    epoch: Instant,
}
impl Sink {
    pub(super) fn push(&self, samples: &[f32]) -> Result<(), String> {
        if samples.len() > 65536 || self.failed.load(Ordering::Relaxed) {
            self.failed.store(true, Ordering::Relaxed);
            return Err(failure());
        }
        let micros = self.epoch.elapsed().as_micros().min(u64::MAX as u128) as u64;
        if self
            .sender
            .try_send(Message::Samples(micros, samples.to_vec()))
            .is_err()
        {
            self.failed.store(true, Ordering::Relaxed);
            return Err(failure());
        }
        Ok(())
    }
}
enum SessionMode {
    Capture {
        sink: Sink,
        worker: std::thread::JoinHandle<Result<(), String>>,
    },
    Replay {
        call: Call,
        action: String,
        error: Option<String>,
    },
}
pub(super) struct Session(SessionMode);
impl Session {
    pub(super) fn finish(self, action: &str, error: Option<String>) -> Result<(), String> {
        match self.0 {
            SessionMode::Capture { sink, worker } => {
                let sent = sink.sender.send(Message::End(action.into(), error));
                let joined = worker.join().map_err(|_| failure())?;
                sent.map_err(|_| failure())?;
                joined
            }
            SessionMode::Replay {
                call,
                action: expected,
                error: expected_error,
            } => {
                if action != expected || error != expected_error {
                    call.invalidate();
                    return Err(failure());
                }
                call.finish().map_err(|_| failure())
            }
        }
    }
}
fn append(call: &Call, frame: Frame) -> Result<(), String> {
    let value = serde_json::to_value(frame).map_err(|_| failure())?;
    call.push(value).map_err(|_| failure())
}
fn encode(samples: &[f32]) -> String {
    STANDARD.encode(
        samples
            .iter()
            .flat_map(|sample| sample.to_le_bytes())
            .collect::<Vec<_>>(),
    )
}
fn decode(value: &str) -> Result<Vec<f32>, String> {
    let bytes = STANDARD.decode(value).map_err(|_| failure())?;
    if bytes.len() % 4 != 0 || bytes.len() > 65536 * 4 {
        return Err(failure());
    }
    Ok(bytes
        .as_chunks::<4>()
        .0
        .iter()
        .map(|sample| f32::from_le_bytes(*sample))
        .collect())
}
pub(super) fn start(device: Option<&str>) -> Result<Capture, String> {
    let Some(call) = workflow::microphone_call().map_err(|_| failure())? else {
        return audio::start_live(device, None);
    };
    if let Some(frames) = call.recorded() {
        return replay(call, frames);
    }
    let (sender, receiver) = mpsc::sync_channel(64);
    let sink = Sink {
        sender,
        failed: Arc::new(AtomicBool::new(false)),
        epoch: Instant::now(),
    };
    let mut capture = match audio::start_live(device, Some(sink.clone())) {
        Ok(capture) => capture,
        Err(error) => {
            append(
                &call,
                Frame::Opened {
                    result: Err(error.clone()),
                },
            )?;
            call.finish().map_err(|_| failure())?;
            return Err(error);
        }
    };
    append(
        &call,
        Frame::Opened {
            result: Ok(capture.sample_rate()),
        },
    )?;
    let failed = sink.failed.clone();
    let worker = std::thread::spawn(move || {
        let result = (|| {
            loop {
                let message = receiver.recv().map_err(|_| failure())?;
                if failed.load(Ordering::Relaxed) {
                    return Err(failure());
                }
                match message {
                    Message::Samples(micros, samples) => append(
                        &call,
                        Frame::Samples {
                            micros,
                            pcm: encode(&samples),
                        },
                    )?,
                    Message::End(action, error) => {
                        append(&call, Frame::End { action, error })?;
                        break;
                    }
                }
            }
            Ok(())
        })();
        if result.is_err() {
            call.invalidate();
        }
        call.finish().map_err(|_| failure())?;
        result
    });
    capture.attach_workflow(Session(SessionMode::Capture { sink, worker }));
    Ok(capture)
}
fn replay(call: Call, frames: Vec<(u64, serde_json::Value)>) -> Result<Capture, String> {
    let result = replay_checked(frames);
    let (rate, samples, action, error, ready_micros) = match result {
        Ok(value) => value,
        Err(error) => {
            call.invalidate();
            return Err(error);
        }
    };
    std::thread::sleep(Duration::from_micros(ready_micros));
    let rate = match rate {
        Ok(rate) => rate,
        Err(error) => {
            call.finish().map_err(|_| failure())?;
            return Err(error);
        }
    };
    let source_error = error.clone();
    let mut capture = audio::replay_capture(rate, move |buffers, stop| {
        let epoch = Instant::now();
        let mut stopped = false;
        for (time, samples) in samples {
            let elapsed = epoch.elapsed().as_micros() as u64 + ready_micros;
            if !stopped
                && stop
                    .recv_timeout(Duration::from_micros(time.saturating_sub(elapsed)))
                    .is_ok()
            {
                stopped = true;
            }
            // The recorded stop boundary defines the complete input. Flush its
            // remaining samples when the replay action arrives slightly earlier.
            audio::push(&buffers, &samples, rate as usize * 120);
        }
        if let Some(error) = source_error {
            audio::replay_error(&buffers, error);
        }
        if !stopped {
            let _ = stop.recv();
        }
    });
    capture.attach_workflow(Session(SessionMode::Replay {
        call,
        action,
        error,
    }));
    Ok(capture)
}
type Recording = (
    Result<u32, String>,
    Vec<(u64, Vec<f32>)>,
    String,
    Option<String>,
    u64,
);
fn replay_checked(frames: Vec<(u64, serde_json::Value)>) -> Result<Recording, String> {
    let mut frames = frames.into_iter();
    let (ready, first) = frames.next().ok_or_else(failure)?;
    if ready > 120_000_000 {
        return Err(failure());
    }
    let Frame::Opened { result } = serde_json::from_value(first).map_err(|_| failure())? else {
        return Err(failure());
    };
    if result.is_err() {
        if frames.next().is_some() {
            return Err(failure());
        }
        return Ok((result, Vec::new(), "failed".into(), None, ready));
    }
    if result
        .as_ref()
        .is_ok_and(|rate| *rate == 0 || *rate > 384000)
    {
        return Err(failure());
    }
    let mut samples = Vec::new();
    let mut previous = 0;
    let mut terminal = None;
    for (_, payload) in frames {
        if terminal.is_some() {
            return Err(failure());
        }
        match serde_json::from_value(payload).map_err(|_| failure())? {
            Frame::Samples { micros, pcm } => {
                if micros < previous || micros > 86_400_000_000 {
                    return Err(failure());
                }
                samples.push((micros, decode(&pcm)?));
                previous = micros;
            }
            Frame::End { action, error } if action == "finish" || action == "cancel" => {
                terminal = Some((action, error))
            }
            _ => return Err(failure()),
        }
    }
    let (action, error) = terminal.ok_or_else(failure)?;
    Ok((result, samples, action, error, ready))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn pcm_preserves_exact_f32_bits_and_wave_encoding() {
        let samples = vec![-0.0, 0.1, -0.25, 1.0, f32::from_bits(1)];
        let restored = decode(&encode(&samples)).unwrap();
        assert_eq!(
            samples.iter().map(|v| v.to_bits()).collect::<Vec<_>>(),
            restored.iter().map(|v| v.to_bits()).collect::<Vec<_>>()
        );
        assert_eq!(
            super::super::wav::encode_wav(&samples, 24000).unwrap(),
            super::super::wav::encode_wav(&restored, 24000).unwrap()
        );
    }
    #[test]
    fn bounded_sink_fails_instead_of_dropping_samples() {
        let (sender, _receiver) = mpsc::sync_channel(1);
        let sink = Sink {
            sender,
            failed: Arc::new(AtomicBool::new(false)),
            epoch: Instant::now(),
        };
        sink.push(&[0.1]).unwrap();
        assert!(sink.push(&[0.2]).is_err());
        assert!(sink.failed.load(Ordering::Relaxed));
    }
    #[test]
    fn mic_recording_requires_ordered_pcm_and_an_explicit_stop_boundary() {
        let start = serde_json::to_value(Frame::Opened { result: Ok(24000) }).unwrap();
        let pcm = serde_json::to_value(Frame::Samples {
            micros: 10,
            pcm: encode(&[0.1]),
        })
        .unwrap();
        assert!(replay_checked(vec![(0, start.clone()), (10, pcm.clone())]).is_err());
        let end = serde_json::to_value(Frame::End {
            action: "finish".into(),
            error: None,
        })
        .unwrap();
        let value = replay_checked(vec![(0, start), (10, pcm), (20, end)]).unwrap();
        assert_eq!(value.0.unwrap(), 24000);
        assert_eq!(value.1, vec![(10, vec![0.1])]);
        assert_eq!(value.2, "finish");
    }
    #[test]
    fn replay_stop_joins_pcm_delivery_before_encoding_identical_wav() {
        let expected = vec![0.1, -0.3, 0.5, -0.0];
        let samples = expected.clone();
        let capture = audio::replay_capture(24000, move |buffers, stop| {
            stop.recv().unwrap();
            audio::push(&buffers, &samples[..2], 24000 * 120);
            audio::push(&buffers, &samples[2..], 24000 * 120);
        });
        assert_eq!(
            capture.finish().unwrap(),
            super::super::wav::encode_wav(&expected, 24000).unwrap()
        );
    }
}
