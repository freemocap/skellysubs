//! Provider-neutral hosted/custom speech wire protocol.
use crate::ai::audio::{SpeechInput, SpeechOutcome};
use crate::ai::connections::access::{ResolvedTarget, response_bytes};
use crate::ai::hosted;
use crate::model::{AppError, ConnectionRoute, ErrorCode, Result};
#[cfg(test)]
use base64::{Engine, engine::general_purpose::STANDARD};

pub(in crate::ai) fn validate(input: &SpeechInput) -> Result<()> {
    if crate::configuration::speech::primary(&input.language_tag).is_none()
        || input.text.trim().is_empty()
        || input.text.len() > 16_384
        || input.text.contains('\0')
        || input.language.trim().is_empty()
        || input.language.len() > 256
        || input
            .language
            .chars()
            .any(|c| c.is_control() || matches!(c, '[' | ']'))
    {
        return Err(AppError::new(
            ErrorCode::Validation,
            "Speech requires bounded source text and a valid language variety.",
        ));
    }
    Ok(())
}

fn decode(bytes: &[u8], source: &str, outcome: &mut SpeechOutcome) -> Result<Vec<u8>> {
    let value: serde_json::Value = serde_json::from_slice(bytes).map_err(|cause| {
        crate::diagnostics::response::json_context(
            &cause,
            "speech_json",
            AppError::new(ErrorCode::Provider, "Invalid speech response JSON."),
        )
    })?;
    outcome.diagnostics = Some(crate::diagnostics::response::metadata(&value, &[]));
    outcome.provider_id = value["usage"]["request_id"].as_str().map(str::to_owned);
    if value["version"] != 3 {
        return Err(crate::diagnostics::response::invalid(
            "speech",
            "version",
            "version 3 provider response",
            &value,
        ));
    }
    let mut decoder = super::speech_stream::Decoder::new(source);
    for record in [
        serde_json::json!({"version":3,"seq":0,"type":"start","format":"pcm_s16le","sample_rate":24000,"channels":1}),
        serde_json::json!({"version":3,"seq":1,"type":"audio","response":value["response"]}),
        serde_json::json!({"version":3,"seq":2,"type":"complete","usage":value["usage"]}),
    ] {
        decoder.record_value(record, outcome, &|_, _, _| Ok(()))?;
    }
    decoder.finish(outcome, &|_, _, _| Ok(()))
}

#[cfg(test)]
pub(in crate::ai) async fn synthesize(
    client: &reqwest::Client,
    target: &ResolvedTarget,
    key: &str,
    input: &SpeechInput,
    install: &str,
) -> SpeechOutcome {
    synthesize_stream(client, target, key, input, install, &|_, _, _| Ok(())).await
}

pub(in crate::ai) async fn synthesize_stream(
    client: &reqwest::Client,
    target: &ResolvedTarget,
    key: &str,
    input: &SpeechInput,
    install: &str,
    emit: &crate::ai::audio::SpeechSink<'_>,
) -> SpeechOutcome {
    #[cfg(feature = "workflow-capture")]
    {
        super::workflow::audio::speech(client, target, key, input, install, emit).await
    }
    #[cfg(not(feature = "workflow-capture"))]
    {
        synthesize_stream_live(client, target, key, input, install, emit).await
    }
}

pub(super) async fn synthesize_stream_live(
    client: &reqwest::Client,
    target: &ResolvedTarget,
    key: &str,
    input: &SpeechInput,
    install: &str,
    emit: &crate::ai::audio::SpeechSink<'_>,
) -> SpeechOutcome {
    let mut outcome = SpeechOutcome::empty();
    outcome.audio = async {
    #[cfg(feature = "workflow-capture")]
    crate::ai::transport::workflow::require_live_network()?;
        validate(input)?;
        // Display labels are not provider delivery instructions. The former
        // accent prefix caused v4 Turbo to repeat speech, including in complete
        // responses. Preserve source text for every supported model/variety.
        let prepared = &input.text;
        let limit = if target.model == "eleven_v4_turbo" { 2_000 } else { 5_000 };
        if prepared.len() > 16_384 || prepared.chars().count() > limit {
            return Err(AppError::new(ErrorCode::Validation, "Prepared speech exceeds the provider input limit."));
        }
        let mut request = client
            .post(&target.url)
            .header(reqwest::header::ACCEPT, "application/x-ndjson")
            .json(&serde_json::json!({"model": target.model, "text": prepared, "language_code": null}));
        if !key.is_empty() {
            request = request.bearer_auth(key);
        }
        if target.route == ConnectionRoute::Hosted {
            request = hosted::identity(request, install);
        }
        let response = request
            .send()
            .await
            .map_err(|e| crate::diagnostics::response::network(&e, "speech_request"))?;
        let http = crate::diagnostics::response::headers(&response);
        let status = response.status();
        outcome.diagnostics = Some(serde_json::json!({"http":http}));
        let streaming = response.headers().get(reqwest::header::CONTENT_TYPE)
            .and_then(|v| v.to_str().ok()).is_some_and(|v| v.split(';').next().unwrap_or("").trim() == "application/x-ndjson");
        let result = if status.is_success() && streaming {
            super::speech_stream::receive(response, &input.text, &mut outcome, emit).await
        } else {
            let bytes = if !status.is_success()
                && (target.route == ConnectionRoute::Hosted
                    || matches!(status.as_u16(), 400 | 409 | 422 | 502 | 503))
            {
                hosted::body_with_private(response, &[key, &input.text]).await
            } else {
                response_bytes(response, "Speech", target.route, 8 * 1024 * 1024).await
            }
            .map_err(|mut error| {
                if !status.is_client_error() {
                    error.code = ErrorCode::UnknownOutcome;
                }
                error
            })?;
            decode(&bytes, &input.text, &mut outcome)
        };
        if outcome.alignment.as_ref().is_some_and(|a| a.source_text != input.text) {
            outcome.alignment = None;
            outcome.diagnostics.as_mut().unwrap()["alignmentValidation"] = serde_json::json!({
                "status":"unavailable", "reason":"source_mismatch", "stage":"speech_alignment"});
        }
        outcome
            .diagnostics
            .get_or_insert_with(|| serde_json::json!({}))["http"] = http;
        result
    }
    .await;
    if let Some(resolution) = &target.audio_resolution {
        outcome
            .diagnostics
            .get_or_insert_with(|| serde_json::json!({}))["routing"] =
            serde_json::json!(resolution);
        if let Err(error) = &mut outcome.audio {
            error
                .diagnostics
                .get_or_insert_with(|| serde_json::json!({}))["routing"] =
                serde_json::json!(resolution);
        }
    }
    outcome.provider_id = outcome
        .provider_id
        .as_ref()
        .map(|value| crate::diagnostics::response::scrub(value, &[key, &input.text]));
    outcome.actual_model = outcome
        .actual_model
        .as_ref()
        .map(|value| crate::diagnostics::response::scrub(value, &[key, &input.text]));
    outcome.diagnostics = outcome
        .diagnostics
        .as_ref()
        .map(|v| crate::diagnostics::response::metadata(v, &[key, &input.text]));
    if let Err(error) = &mut outcome.audio {
        error.message = crate::diagnostics::response::scrub(&error.message, &[key, &input.text]);
        error.diagnostics = error
            .diagnostics
            .as_ref()
            .map(|v| crate::diagnostics::response::metadata(v, &[key, &input.text]));
    }
    outcome
}

#[cfg(test)]
mod tests {
    use super::*;
    fn target() -> ResolvedTarget {
        ResolvedTarget {
            audio_resolution: None,
            route: ConnectionRoute::Custom,
            revision: 1,
            url: "http://localhost/v1/audio/speech".into(),
            model: "eleven_v3".into(),
            credential: None,
        }
    }
    fn body() -> serde_json::Value {
        serde_json::json!({"version":3,"response":{"audio_base64":STANDARD.encode([100u8,0])},
            "usage":{"requested_model":"eleven_v3","actual_model":null,"provider":"elevenlabs","request_id":"receipt","cost_micros":null,
                "allowance_micros":100,"allowance_basis":"estimate"}})
    }

    #[test]
    fn estimated_allowance_is_not_reported_as_actual_cost() {
        let mut outcome = SpeechOutcome::empty();
        let wav = decode(
            &serde_json::to_vec(&body()).unwrap(),
            "source",
            &mut outcome,
        )
        .unwrap();
        assert!(wav.starts_with(b"RIFF"));
        assert_eq!(outcome.provider_id.as_deref(), Some("receipt"));
        assert_eq!(outcome.finish_reason.as_deref(), Some("stop"));
        assert!(outcome.cost_micros.is_none());
        assert!(outcome.input_tokens.is_none());
    }
    #[test]
    fn malformed_audio_retains_receipt_without_publication() {
        let mut value = body();
        value["response"]["audio_base64"] = serde_json::json!("YWJj");
        let mut outcome = SpeechOutcome::empty();
        assert!(decode(&serde_json::to_vec(&value).unwrap(), "source", &mut outcome).is_err());
        assert_eq!(outcome.provider_id.as_deref(), Some("receipt"));
        value = body();
        value["usage"]["requested_model"] = serde_json::json!("wrong-model");
        assert!(decode(&serde_json::to_vec(&value).unwrap(), "source", &mut outcome).is_ok());
    }
    #[test]
    fn playable_audio_does_not_require_a_complete_usage_receipt() {
        for usage in [
            serde_json::Value::Null,
            serde_json::json!("unavailable"),
            serde_json::json!({"requested_model":"different-model","cost_micros":"unknown"}),
        ] {
            let mut value = body();
            value["usage"] = usage;
            let mut outcome = SpeechOutcome::empty();
            assert!(decode(&serde_json::to_vec(&value).unwrap(), "source", &mut outcome).is_ok());
            assert_eq!(outcome.cost_micros, None);
        }
    }
    #[tokio::test]
    async fn sends_only_internal_request_with_server_token() {
        for model in ["eleven_v3", "eleven_v4_turbo"] {
            for text in [
                "Trains can be faster.",
                "No, nunca había escuchado de ellos.",
                "مرحبا",
                "你好",
                "cafe\u{301}",
            ] {
                assert_source_request(model, text).await;
            }
        }
    }

    async fn assert_source_request(model: &str, source: &str) {
        use std::io::{Read, Write};
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let mut target = target();
        target.model = model.into();
        target.url = format!("http://{}/v1/audio/speech", listener.local_addr().unwrap());
        let body = body().to_string();
        let expected = serde_json::json!({"model":model,"text":source,"language_code":null});
        let worker = std::thread::spawn(move || {
            let (mut socket, _) = listener.accept().unwrap();
            socket
                .set_read_timeout(Some(std::time::Duration::from_secs(5)))
                .unwrap();
            let mut request = Vec::new();
            loop {
                let mut buffer = [0; 4096];
                let count = socket.read(&mut buffer).unwrap();
                assert!(count > 0);
                request.extend_from_slice(&buffer[..count]);
                let text = String::from_utf8_lossy(&request);
                if let Some((headers, payload)) = text.split_once("\r\n\r\n") {
                    let length: usize = headers
                        .lines()
                        .find_map(|line| {
                            line.to_lowercase()
                                .strip_prefix("content-length: ")
                                .and_then(|n| n.parse().ok())
                        })
                        .unwrap();
                    if payload.len() >= length {
                        break;
                    }
                }
            }
            let request = String::from_utf8(request).unwrap();
            assert!(request.starts_with("POST /v1/audio/speech"));
            assert!(
                request
                    .to_lowercase()
                    .contains("authorization: bearer server-token")
            );
            let value: serde_json::Value =
                serde_json::from_str(request.split_once("\r\n\r\n").unwrap().1).unwrap();
            assert_eq!(value, expected);
            write!(
                socket,
                "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                body.len(),
                body
            )
            .unwrap();
        });
        let outcome = synthesize(
            &crate::ai::transport::provider::client().unwrap(),
            &target,
            "server-token",
            &SpeechInput {
                language_tag: "es-MX".into(),
                text: source.into(),
                voice: "alloy".into(),
                language: "Spanish — Mexico".into(),
            },
            "install",
        )
        .await;
        worker.join().unwrap();
        assert!(outcome.audio.is_ok());
    }
}

#[cfg(test)]
mod relay_tests {
    use super::*;
    #[test]
    fn full_response_is_not_subject_to_stream_frame_limit() {
        let pcm = vec![0u8; 3_200_000];
        let value = serde_json::json!({"version":3,"response":{"audio_base64":STANDARD.encode(&pcm)},"usage":{"request_id":"whole"}});
        let mut outcome = SpeechOutcome::empty();
        let wav = decode(&serde_json::to_vec(&value).unwrap(), "source", &mut outcome).unwrap();
        assert_eq!(wav.len(), pcm.len() + 44);
        assert_eq!(outcome.provider_id.as_deref(), Some("whole"));
    }
}
