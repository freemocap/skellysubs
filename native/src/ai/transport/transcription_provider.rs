//! Access transport. Provider request/response conversion lives in adapters.
use super::transcription_adapters::Adapter;
use crate::ai::audio::{TranscriptionLanguage, TranscriptionOutcome, TranscriptionRequest};
use crate::ai::connections::access::{ResolvedTarget, response_bytes};
use crate::ai::hosted;
use crate::model::{AppError, ConnectionRoute, ErrorCode, Result};
fn error(message: impl Into<String>) -> AppError {
    AppError::new(ErrorCode::Validation, message)
}
pub(in crate::ai) fn validate_language(
    _target: &ResolvedTarget,
    language: &TranscriptionLanguage,
) -> Result<String> {
    Adapter::language(language)
}
#[cfg(test)]
fn transcription_response(bytes: &[u8], verbose: bool) -> Result<TranscriptionOutcome> {
    let _ = verbose;
    Adapter::decode(bytes)
}
#[cfg(test)]
fn test_language(tag: &str) -> TranscriptionLanguage {
    TranscriptionLanguage {
        language_id: tag.into(),
        variety_id: "default".into(),
        language_tag: tag.into(),
    }
}
pub(in crate::ai) async fn transcribe(
    client: &reqwest::Client,
    target: &ResolvedTarget,
    key: &str,
    input: TranscriptionRequest,
    install: &str,
) -> Result<TranscriptionOutcome> {
    #[cfg(feature = "workflow-capture")]
    {
        super::workflow::audio::transcription(client, target, key, input, install).await
    }
    #[cfg(not(feature = "workflow-capture"))]
    {
        transcribe_live(client, target, key, input, install).await
    }
}

pub(super) async fn transcribe_live(
    client: &reqwest::Client,
    target: &ResolvedTarget,
    key: &str,
    input: TranscriptionRequest,
    install: &str,
) -> Result<TranscriptionOutcome> {
    let mut outcome = execute(client, target, key, input, install).await;
    let diagnostics = match &mut outcome {
        Ok(value) => &mut value.diagnostics,
        Err(error) => &mut error.diagnostics,
    };
    let metadata = diagnostics.get_or_insert_with(|| serde_json::json!({}));
    metadata["requested_model"] = serde_json::json!(target.model);
    if let Some(resolution) = &target.audio_resolution {
        metadata["routing"] = serde_json::json!(resolution);
    }
    outcome
}

async fn execute(
    client: &reqwest::Client,
    target: &ResolvedTarget,
    key: &str,
    input: TranscriptionRequest,
    install: &str,
) -> Result<TranscriptionOutcome> {
    #[cfg(feature = "workflow-capture")]
    crate::ai::transport::workflow::require_live_network()?;
    let private_context = input.context.clone().unwrap_or_default();
    let wav = &input.wav;
    if wav.is_empty() || wav.len() > 25 * 1024 * 1024 {
        return Err(error("Recording must contain audio and be at most 25 MB."));
    }
    let duration = hound::WavReader::new(std::io::Cursor::new(&input.wav))
        .ok()
        .map(|reader| reader.duration() as f64 / reader.spec().sample_rate as f64);
    let form = Adapter::form(&target.model, input, key)?;
    let request = client.post(&target.url);
    let request = if key.is_empty() {
        request
    } else {
        request.bearer_auth(key)
    };
    let request = if target.route == ConnectionRoute::Hosted {
        hosted::identity(request, install)
    } else {
        request
    };
    let response = request
        .multipart(form)
        .send()
        .await
        .map_err(|e| crate::diagnostics::response::network(&e, "transcription_request"))?;
    let http = crate::diagnostics::response::headers(&response);
    let status = response.status();
    let bytes = if target.route == ConnectionRoute::Hosted
        || (target.route == ConnectionRoute::Custom
            && matches!(status.as_u16(), 400 | 422 | 502 | 503))
    {
        hosted::body_with_private(response, &[key, &private_context]).await
    } else {
        response_bytes(response, "Transcription", target.route, 1_048_576).await
    }
    .map_err(|mut error| {
        if !status.is_client_error() {
            error.code = ErrorCode::UnknownOutcome;
        }
        error
    })?;
    let mut result = Adapter::decode_with_duration(&bytes, duration);
    match &mut result {
        Ok(value) => {
            value
                .diagnostics
                .get_or_insert_with(|| serde_json::json!({}))["http"] = http;
        }
        Err(error) => {
            error
                .diagnostics
                .get_or_insert_with(|| serde_json::json!({}))["http"] = http;
        }
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ai::transport::provider;
    use std::time::Duration;
    #[tokio::test]
    async fn multipart_custom_transcription_sends_only_selected_auth_and_contract() {
        use std::io::{Read, Write};
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let url = format!(
            "http://{}/v1/audio/transcriptions",
            listener.local_addr().unwrap()
        );
        let worker = std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            stream
                .set_read_timeout(Some(Duration::from_secs(5)))
                .unwrap();
            let mut bytes = Vec::new();
            let mut buf = [0u8; 4096];
            let end = loop {
                let n = stream.read(&mut buf).unwrap();
                assert!(n > 0);
                bytes.extend_from_slice(&buf[..n]);
                if let Some(i) = bytes.windows(4).position(|w| w == b"\r\n\r\n") {
                    break i + 4;
                }
            };
            let headers = String::from_utf8_lossy(&bytes[..end]).to_lowercase();
            let size: usize = headers
                .lines()
                .find_map(|l| l.strip_prefix("content-length:"))
                .unwrap()
                .trim()
                .parse()
                .unwrap();
            while bytes.len() < end + size {
                let n = stream.read(&mut buf).unwrap();
                assert!(n > 0);
                bytes.extend_from_slice(&buf[..n]);
            }
            assert!(headers.starts_with("post /v1/audio/transcriptions "));
            assert!(!headers.contains("authorization:"));
            assert!(!headers.contains("x-skelly"));
            let body = String::from_utf8_lossy(&bytes[end..]);
            for expected in [
                "name=\"model\"",
                "custom-whisper",
                "name=\"language\"",
                "es",
                "Spanish — Spain",
                "name=\"prompt\"",
                "filename=\"audio.wav\"",
                "RIFF-test-audio",
            ] {
                assert!(body.contains(expected), "{expected}");
            }
            assert!(body.contains("name=\"response_format\"\r\n\r\nverbose_json"));
            assert!(body.contains("timestamp_granularities"));
            let body = r#"{"version":3,"response":{"text":"Hola, ¿cómo estás?"}}"#;
            write!(
                stream,
                "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                body.len()
            )
            .unwrap();
        });
        let target = ResolvedTarget {
            audio_resolution: None,
            route: ConnectionRoute::Custom,
            revision: 1,
            url,
            model: "custom-whisper".into(),
            credential: None,
        };
        let (writer, events) = crate::ai::content::tests::capture_events();
        let result = crate::ai::content::capture::scope(
            writer,
            transcribe(
                &provider::client().unwrap(),
                &target,
                "",
                TranscriptionRequest {
                    wav: b"RIFF-test-audio".to_vec(),
                    language: test_language("es"),
                    context: Some("Spanish — Spain".into()),
                },
                "private-install-id",
            ),
        )
        .await
        .unwrap();
        assert_eq!(
            crate::ai::content::tests::captured(&events, "transcription.audio"),
            b"RIFF-test-audio"
        );
        let fields = String::from_utf8(crate::ai::content::tests::captured(
            &events,
            "transcription.multipart-fields.request",
        ))
        .unwrap();
        assert!(
            fields.contains("custom-whisper")
                && fields.contains("Spanish — Spain")
                && fields.contains("timestamp_granularities")
        );
        assert_eq!(
            crate::ai::content::tests::captured(&events, "transcription.multipart-fields.response"),
            r#"{"version":3,"response":{"text":"Hola, ¿cómo estás?"}}"#.as_bytes()
        );
        assert_eq!(result.result.text, "Hola, ¿cómo estás?");
        assert!(result.result.timing.is_none());
        worker.join().unwrap();
    }

    #[test]
    fn text_only_transcription_preserves_script_without_fabricating_evidence() {
        let response = transcription_response(
            "{\"version\":3,\"response\":{\"text\":\"അത് നല്ലതാണ്.\"}}".as_bytes(),
            false,
        )
        .unwrap();
        assert_eq!(response.result.text, "അത് നല്ലതാണ്.");
        assert!(response.result.timing.is_none());
        assert_eq!(
            transcription_response(br#"{"version":3,"response":{"text":" "}}"#, false)
                .unwrap()
                .result
                .text,
            " "
        );
        assert!(transcription_response(br#"{"error":"failure"}"#, false).is_err());
    }
}

#[cfg(test)]
mod service_timing_tests {
    use super::*;
    #[test]
    fn native_timing_projection_preserves_text_and_rejects_invalid_evidence() {
        let mut value = serde_json::json!({"version":3,"response":{"text":"നമസ്കാരം",
            "duration":1.0,"words":[{"word":"നമസ്കാരം","start":0.1,"end":0.9}]}});
        let result = transcription_response(&serde_json::to_vec(&value).unwrap(), false).unwrap();
        assert_eq!(result.result.timing.unwrap().words.len(), 1);
        value["response"]["words"][0]["end"] = serde_json::json!(2.0);
        let out = transcription_response(&serde_json::to_vec(&value).unwrap(), false).unwrap();
        assert_eq!(out.result.text, "നമസ്കാരം");
        assert!(out.result.timing.is_none());
        assert_eq!(
            out.diagnostics.unwrap()["timing"]["reason"],
            "invalid_provider_timing"
        );
    }
}
