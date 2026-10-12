//! SkellySpeak transcription wire conversion. Providers are owned by the server.
use crate::ai::audio::{
    TranscriptionLanguage, TranscriptionOutcome, TranscriptionRequest, TranscriptionResult,
};
use crate::model::{AppError, ErrorCode, Result};
use crate::speech::analysis::fluency::TranscriptTiming;
use serde_json::Value;
#[cfg(test)]
use serde_json::json;
pub(super) struct Adapter;
fn invalid(message: &str) -> AppError {
    AppError::new(ErrorCode::Validation, message)
}
impl Adapter {
    pub fn language(language: &TranscriptionLanguage) -> Result<String> {
        let tag = &language.language_tag;
        if language.language_id.is_empty()
            || language.variety_id.is_empty()
            || tag.len() > 80
            || !tag
                .split('-')
                .all(|p| !p.is_empty() && p.bytes().all(|c| c.is_ascii_alphanumeric()))
        {
            return Err(invalid(
                "Transcription requires an explicit language identity.",
            ));
        }
        let code = tag.split('-').next().unwrap_or_default();
        if !(2..=3).contains(&code.len()) || !code.bytes().all(|c| c.is_ascii_lowercase()) {
            return Err(invalid("Invalid transcription language code."));
        }
        Ok(tag.clone())
    }
    pub fn form(
        model: &str,
        input: TranscriptionRequest,
        key: &str,
    ) -> Result<reqwest::multipart::Form> {
        let tag = Self::language(&input.language)?;
        let catalog = crate::configuration::speech::Catalog::bundled();
        let primary = crate::configuration::speech::primary(&tag)
            .ok_or_else(|| invalid("Invalid transcription language."))?;
        let definition = catalog.models.get(model);
        let language = definition
            .and_then(|d| catalog.language_sets.get(&d.languages))
            .and_then(|languages| languages.get(primary))
            .map(String::as_str)
            .unwrap_or(primary);
        if definition.is_some_and(|d| !d.allow_unlisted_languages)
            && definition
                .and_then(|d| catalog.language_sets.get(&d.languages))
                .and_then(|set| set.get(primary))
                .is_none()
        {
            return Err(invalid(
                "Selected transcription model does not declare this language.",
            ));
        }
        let word_provider = definition.is_some_and(|d| d.provider == "elevenlabs");
        crate::ai::content::capture::source("transcription.audio", "audio/wav", &input.wav, "")?;
        let mut fields = vec![
            ("model", model.to_owned()),
            ("language", language.to_owned()),
        ];
        let mut form = reqwest::multipart::Form::new()
            .part(
                "file",
                reqwest::multipart::Part::bytes(input.wav)
                    .file_name("audio.wav")
                    .mime_str("audio/wav")
                    .map_err(|_| invalid("Invalid audio type."))?,
            )
            .text("model", model.to_owned())
            .text("language", language.to_owned());
        if word_provider {
            for (key, value) in [
                ("timestamps_granularity", "word"),
                ("tag_audio_events", "false"),
                ("diarize", "false"),
                ("no_verbatim", "false"),
            ] {
                fields.push((key, value.to_owned()));
                form = form.text(key, value);
            }
        } else {
            fields.extend([
                ("response_format", "verbose_json".into()),
                ("timestamp_granularities[]", "word".into()),
                ("timestamp_granularities[]", "segment".into()),
            ]);
            form = form
                .text("response_format", "verbose_json")
                .text("timestamp_granularities[]", "word")
                .text("timestamp_granularities[]", "segment");
            if let Some(context) = input.context {
                let context = format!("{tag}\n{context}");
                let end = (0..=context.len().min(224))
                    .rev()
                    .find(|&n| context.is_char_boundary(n))
                    .unwrap();
                fields.push(("prompt", context[..end].to_owned()));
                form = form.text("prompt", context[..end].to_owned());
            }
        }
        crate::ai::content::capture::request(
            "transcription.multipart-fields",
            "application/json",
            &serde_json::to_vec(&fields)?,
            key,
        )?;
        Ok(form)
    }
    #[cfg(test)]
    pub fn decode(bytes: &[u8]) -> Result<TranscriptionOutcome> {
        Self::decode_with_duration(bytes, None)
    }
    pub fn decode_with_duration(
        bytes: &[u8],
        duration: Option<f64>,
    ) -> Result<TranscriptionOutcome> {
        let value: Value = serde_json::from_slice(bytes).map_err(|cause| {
            crate::diagnostics::response::json_context(
                &cause,
                "transcription_json",
                invalid("Invalid transcription JSON."),
            )
        })?;
        if value["version"] != 3 || !value["response"].is_object() {
            return Err(crate::diagnostics::response::invalid(
                "transcription",
                "version/response",
                "version 3 provider response",
                &value,
            ));
        }
        let raw = &value["response"];
        let failure = || {
            crate::diagnostics::response::invalid(
                "transcription",
                "response.text",
                "bounded transcript text",
                &value,
            )
        };
        let text = raw["text"].as_str().ok_or_else(failure)?.to_owned();
        if text.chars().count() > 20000 || text.contains('\0') {
            return Err(failure());
        }
        let mut diagnostics = crate::diagnostics::response::metadata(&value, &[]);
        let duration = duration.or_else(|| raw["duration"].as_f64()).unwrap_or(0.0);
        let timing = match super::transcription_timing::decode(raw, duration) {
            Ok(timing) => {
                serde_json::from_value::<Option<TranscriptTiming>>(timing).map_err(|_| failure())?
            }
            Err(reason) => {
                diagnostics["timing"] = reason;
                None
            }
        };
        // Compute before redacting/truncating provider lists for diagnostics.
        if let Some(summary) = super::transcription_confidence::from_response(raw) {
            diagnostics["transcription_confidence"] = summary;
        }
        let result = TranscriptionResult { text, timing };

        Ok(TranscriptionOutcome {
            result,
            diagnostics: Some(diagnostics),
        })
    }
}

#[cfg(test)]
mod confidence_tests {
    use super::*;

    #[test]
    fn existing_service_evidence_reaches_the_drill_gate() {
        let value = json!({"version":3,"response":{"text":"fixture","segments":[
            {"avg_logprob":-0.05339829,"no_speech_prob":0.0012197495}]}});
        let outcome = Adapter::decode(&serde_json::to_vec(&value).unwrap()).unwrap();
        let mut wav = std::io::Cursor::new(Vec::new());
        let spec = hound::WavSpec {
            channels: 1,
            sample_rate: 8000,
            bits_per_sample: 16,
            sample_format: hound::SampleFormat::Int,
        };
        let mut writer = hound::WavWriter::new(&mut wav, spec).unwrap();
        for sample in 0..8000 {
            writer
                .write_sample(if (1600..4800).contains(&sample) {
                    3000i16
                } else {
                    0
                })
                .unwrap();
        }
        writer.finalize().unwrap();
        let (inspection, _) = crate::speech::analysis::audio_inspection::inspect_wav(
            &wav.into_inner(),
            "r",
            &crate::speech::recording::owner::RecordingOwner::DrillItem("i".into()),
        )
        .unwrap();
        let reliability =
            crate::drill::reliability::assess(&inspection, outcome.diagnostics.as_ref());
        assert!(reliability.accepted);
        assert_eq!(reliability.source, "segment_logprobs");
        assert!((reliability.confidence.unwrap() - 0.9480023574202364).abs() < 1e-8);
    }

    #[test]
    fn confidence_survives_verbose_metadata_limits_without_content() {
        let value = json!({"version":3,"response":{"text":"private transcript",
            "words":vec![json!({"type":"word","text":"private transcript","logprob":0.8f64.ln()});100]}});
        let outcome = Adapter::decode(&serde_json::to_vec(&value).unwrap()).unwrap();
        assert_eq!(outcome.result.text, "private transcript");
        let metadata = outcome.diagnostics.unwrap();
        assert!(
            (metadata["transcription_confidence"]["score"]
                .as_f64()
                .unwrap()
                - 0.8)
                .abs()
                < 1e-12
        );
        assert_eq!(
            metadata["transcription_confidence"]["source"],
            "word_logprobs"
        );
        assert!(
            metadata["transcription_confidence"]
                .get("private_extra")
                .is_none()
        );
        assert!(!metadata.to_string().contains("private transcript"));
    }
}
