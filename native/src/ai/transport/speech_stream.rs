//! Bounded incremental service audio decoding. No partial result is cacheable.
use crate::ai::audio::SpeechOutcome;
use crate::model::{AppError, ErrorCode, Result};
use base64::{Engine, engine::general_purpose::STANDARD};
use serde_json::{Value, json};

const FRAME_LIMIT: usize = 4 * 1024 * 1024;
const WIRE_LIMIT: usize = 16 * 1024 * 1024;

#[derive(Default)]
pub(super) struct Decoder {
    line: Vec<u8>,
    received: usize,
    sequence: u64,
    complete: bool,
    pcm: Vec<u8>,
    source: String,
    timings: super::synthesis_timing::Timings,
}

impl Decoder {
    pub(super) fn new(source: &str) -> Self {
        Self {
            source: source.into(),
            ..Self::default()
        }
    }
    fn invalid(&self, path: &str) -> AppError {
        AppError::new(
            ErrorCode::UnknownOutcome,
            "Speech stream was incomplete or invalid. No automatic retry was made.",
        )
        .with_diagnostics(json!({"stage":"speech_stream", "path":path,
                "expected":"ordered version 3 provider records and one complete terminal",
                "sequence":self.sequence, "received_bytes":self.received,
                "audio_samples":self.pcm.len()/2, "automatic_retries":[]}))
    }

    pub(super) fn feed(
        &mut self,
        bytes: &[u8],
        outcome: &mut SpeechOutcome,
        emit: &crate::ai::audio::SpeechSink<'_>,
    ) -> Result<()> {
        self.received += bytes.len();
        if self.received > WIRE_LIMIT {
            return Err(self.invalid("response_size"));
        }
        for part in bytes.split_inclusive(|b| *b == b'\n') {
            if self.line.len() + part.len() > FRAME_LIMIT {
                return Err(self.invalid("frame_size"));
            }
            self.line.extend_from_slice(part);
            if part.last() == Some(&b'\n') {
                let line = std::mem::take(&mut self.line);
                self.record(&line, outcome, emit)?;
            }
        }
        Ok(())
    }

    fn record(
        &mut self,
        bytes: &[u8],
        outcome: &mut SpeechOutcome,
        emit: &crate::ai::audio::SpeechSink<'_>,
    ) -> Result<()> {
        let value: Value = serde_json::from_slice(bytes).map_err(|e| {
            self.invalid("json")
                .with_diagnostics(json!({"stage":"speech_stream", "line":e.line(),
                "column":e.column(),"audio_samples":self.pcm.len()/2,"automatic_retries":[]}))
        })?;
        self.record_value(value, outcome, emit)
    }

    // Complete JSON responses have a different framing limit than streamed lines.
    // Both paths use identical audio, timing, receipt and sequence validation.
    pub(super) fn record_value(
        &mut self,
        value: Value,
        outcome: &mut SpeechOutcome,
        emit: &crate::ai::audio::SpeechSink<'_>,
    ) -> Result<()> {
        if self.complete
            || value["version"] != 3
            || value["seq"].as_u64() != Some(self.sequence)
            || self.sequence > 4097
        {
            return Err(self.invalid("version/seq/terminal"));
        }
        let kind = value["type"].as_str().unwrap_or_default();
        if self.sequence == 0 {
            if kind != "start"
                || value["format"] != "pcm_s16le"
                || value["sample_rate"] != 24000
                || value["channels"] != 1
            {
                return Err(self.invalid("start"));
            }
        } else {
            // Receipts survive a later audio/terminal validation failure.
            if let Some(receipt) = value.get("receipt").or_else(|| value.get("usage")) {
                if let Some(id) = receipt["request_id"].as_str() {
                    outcome.provider_id = Some(id.into());
                }
                outcome.diagnostics.get_or_insert_with(|| json!({}))["streamReceipt"] =
                    crate::diagnostics::response::metadata(receipt, &[]);
            }
            match kind {
                "audio" => {
                    outcome.diagnostics.get_or_insert_with(|| json!({}))["streamResponse"] =
                        crate::diagnostics::response::metadata(&value["response"], &[]);

                    let encoded = value["response"]["audio_base64"]
                        .as_str()
                        .ok_or_else(|| self.invalid("audio_base64"))?;
                    let pcm = STANDARD
                        .decode(encoded)
                        .map_err(|_| self.invalid("audio_base64"))?;
                    if pcm.is_empty()
                        || pcm.len() % 2 != 0
                        || self.pcm.len() + pcm.len() > crate::speech::delivery::AUDIO_LIMIT - 44
                    {
                        return Err(self.invalid("pcm_size"));
                    }
                    self.timings.append(&value["response"]);
                    self.pcm.extend_from_slice(&pcm);
                    let alignment = self
                        .timings
                        .projection(&self.source, self.pcm.len() as f64 / 48000.0);
                    emit(&pcm, Some(&alignment), outcome)?;
                }
                "complete" => {
                    if self.pcm.is_empty() {
                        return Err(self.invalid("empty_audio"));
                    }
                    outcome.actual_model =
                        value["usage"]["actual_model"].as_str().map(str::to_owned);
                    outcome.cost_micros = value["usage"]["cost_micros"].as_u64();
                    outcome.diagnostics.get_or_insert_with(|| json!({}))["streamCompletion"] =
                        crate::diagnostics::response::metadata(&value, &[]);
                    let duration = self.pcm.len() as f64 / 48000.0;
                    outcome.alignment = Some(self.timings.projection(&self.source, duration));
                    outcome.diagnostics.as_mut().unwrap()["alignmentValidation"] =
                        self.timings.diagnostics(duration);
                    self.complete = true;
                }
                "error" => {
                    let response = crate::diagnostics::response::metadata(&value, &[]);
                    outcome.diagnostics.get_or_insert_with(|| json!({}))["streamError"] =
                        response.clone();
                    return Err(self.invalid("error").with_diagnostics(json!({"stage":"speech_stream",
                        "response":response,"audio_samples":self.pcm.len()/2,"automatic_retries":[]})));
                }
                _ => return Err(self.invalid("type")),
            }
        }
        self.sequence += 1;
        Ok(())
    }

    pub(super) fn finish(
        mut self,
        outcome: &mut SpeechOutcome,
        emit: &crate::ai::audio::SpeechSink<'_>,
    ) -> Result<Vec<u8>> {
        if !self.line.is_empty() {
            let line = std::mem::take(&mut self.line);
            self.record(&line, outcome, emit)?;
        }
        if !self.complete {
            return Err(self.invalid("terminal"));
        }
        let mut wav = std::io::Cursor::new(Vec::new());
        {
            let mut writer = hound::WavWriter::new(
                &mut wav,
                hound::WavSpec {
                    channels: 1,
                    sample_rate: 24000,
                    bits_per_sample: 16,
                    sample_format: hound::SampleFormat::Int,
                },
            )
            .map_err(|_| self.invalid("wav_header"))?;
            for sample in self.pcm.as_chunks::<2>().0 {
                writer
                    .write_sample(i16::from_le_bytes(*sample))
                    .map_err(|_| self.invalid("wav_samples"))?;
            }
            writer
                .finalize()
                .map_err(|_| self.invalid("wav_finalize"))?;
        }
        outcome.finish_reason = Some("stop".into());
        Ok(wav.into_inner())
    }
}

pub(super) async fn receive(
    mut response: reqwest::Response,
    source: &str,
    outcome: &mut SpeechOutcome,
    emit: &crate::ai::audio::SpeechSink<'_>,
) -> Result<Vec<u8>> {
    let mut decoder = Decoder::new(source);
    while let Some(chunk) = response.chunk().await.map_err(|cause| {
        let mut error = crate::diagnostics::response::network(&cause, "speech_stream");
        error.code = ErrorCode::UnknownOutcome;
        error.diagnostics.get_or_insert_with(|| json!({}))["automatic_retries"] = json!([]);
        error.diagnostics.as_mut().unwrap()["audio_samples"] = json!(decoder.pcm.len() / 2);
        error
    })? {
        crate::ai::content::capture::chunk(&chunk)?;
        decoder.feed(&chunk, outcome, emit)?;
    }
    crate::ai::content::capture::complete()?;
    decoder.finish(outcome, emit)
}

#[cfg(test)]
#[path = "speech_stream_tests.rs"]
mod tests;
