use super::{begin, error, without_secret};
use crate::{
    ai::{
        audio::{
            SpeechInput, SpeechOutcome, SpeechSink, TranscriptionOutcome, TranscriptionRequest,
            TranscriptionResult,
        },
        connections::access::ResolvedTarget,
        transport::{service_audio, transcription_provider},
    },
    model::Result,
    speech::alignment::SpeechAlignment,
};
use base64::{Engine, engine::general_purpose::STANDARD};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct SavedSpeech {
    alignment: Option<SpeechAlignment>,
    diagnostics: Option<Value>,
    audio: Result<String>,
    actual_model: Option<String>,
    provider_id: Option<String>,
    input_tokens: Option<u64>,
    output_tokens: Option<u64>,
    cost_micros: Option<u64>,
    finish_reason: Option<String>,
}
impl SavedSpeech {
    fn capture(value: &SpeechOutcome) -> Self {
        Self {
            alignment: value.alignment.clone(),
            diagnostics: value.diagnostics.clone(),
            audio: value
                .audio
                .as_ref()
                .map(|v| STANDARD.encode(v))
                .map_err(Clone::clone),
            actual_model: value.actual_model.clone(),
            provider_id: value.provider_id.clone(),
            input_tokens: value.input_tokens,
            output_tokens: value.output_tokens,
            cost_micros: value.cost_micros,
            finish_reason: value.finish_reason.clone(),
        }
    }
    fn restore(self) -> Result<SpeechOutcome> {
        let audio = match self.audio {
            Ok(value) => Ok(STANDARD.decode(value).map_err(|_| error())?),
            Err(error) => Err(error),
        };
        Ok(SpeechOutcome {
            alignment: self.alignment,
            diagnostics: self.diagnostics,
            audio,
            actual_model: self.actual_model,
            provider_id: self.provider_id,
            input_tokens: self.input_tokens,
            output_tokens: self.output_tokens,
            cost_micros: self.cost_micros,
            finish_reason: self.finish_reason,
        })
    }
}
#[derive(Serialize, Deserialize)]
#[serde(tag = "type", deny_unknown_fields)]
enum SpeechFrame {
    Chunk {
        pcm: String,
        alignment: Option<SpeechAlignment>,
        receipt: SavedSpeech,
    },
    End {
        outcome: SavedSpeech,
    },
}
pub(in crate::ai::transport) async fn speech(
    client: &reqwest::Client,
    target: &ResolvedTarget,
    key: &str,
    input: &SpeechInput,
    install: &str,
    emit: &SpeechSink<'_>,
) -> SpeechOutcome {
    match speech_inner(client, target, key, input, install, emit).await {
        Ok(value) => value,
        Err(error) => {
            let mut value = SpeechOutcome::empty();
            value.audio = Err(error);
            value
        }
    }
}
async fn speech_inner(
    client: &reqwest::Client,
    target: &ResolvedTarget,
    key: &str,
    input: &SpeechInput,
    install: &str,
    emit: &SpeechSink<'_>,
) -> Result<SpeechOutcome> {
    let semantic = json!({"model":target.model,"route":target.route,"text":input.text,"voice":input.voice,"language":input.language,"language_tag":input.language_tag});
    let Some(call) = begin("speech", semantic)? else {
        return Ok(service_audio::synthesize_stream_live(
            client, target, key, input, install, emit,
        )
        .await);
    };
    if let Some(frames) = call.recorded() {
        let mut previous = 0;
        let mut terminal = None;
        for (time, payload) in frames {
            if terminal.is_some() {
                return Err(error());
            }
            call.wait(&mut previous, time).await?;
            if super::originals::restore(&payload)? {
                continue;
            }
            match serde_json::from_value(payload).map_err(|_| error())? {
                SpeechFrame::Chunk {
                    pcm,
                    alignment,
                    receipt,
                } => {
                    let bytes = STANDARD.decode(pcm).map_err(|_| error())?;
                    emit(&bytes, alignment.as_ref(), &receipt.restore()?)?;
                }
                SpeechFrame::End { outcome } => terminal = Some(outcome.restore()?),
            }
        }
        if call.cancelled().is_some() {
            if terminal.is_some() {
                return Err(error());
            }
            return call.require_cancellation(&mut previous).await;
        }
        let result = terminal.ok_or_else(error)?;
        call.finish()?;
        return Ok(result);
    }
    let outcome = call
        .originals(service_audio::synthesize_stream_live(
            client,
            target,
            key,
            input,
            install,
            &|bytes, alignment, receipt| {
                let frame = SpeechFrame::Chunk {
                    pcm: STANDARD.encode(bytes),
                    alignment: alignment.cloned(),
                    receipt: SavedSpeech::capture(receipt),
                };
                call.push(without_secret(serde_json::to_value(frame)?, key))?;
                emit(bytes, alignment, receipt)
            },
        ))
        .await;
    call.push(without_secret(
        serde_json::to_value(SpeechFrame::End {
            outcome: SavedSpeech::capture(&outcome),
        })?,
        key,
    ))?;
    call.finish()?;
    Ok(outcome)
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct SavedTranscription {
    result: TranscriptionResult,
    diagnostics: Option<Value>,
}
pub(in crate::ai::transport) async fn transcription(
    client: &reqwest::Client,
    target: &ResolvedTarget,
    key: &str,
    input: TranscriptionRequest,
    install: &str,
) -> Result<TranscriptionOutcome> {
    let semantic = json!({"model":target.model,"route":target.route,"audio_sha256":format!("{:x}",Sha256::digest(&input.wav)),"language_id":input.language.language_id,"variety_id":input.language.variety_id,"language_tag":input.language.language_tag,"context":input.context});
    let Some(call) = begin("transcription", semantic)? else {
        return transcription_provider::transcribe_live(client, target, key, input, install).await;
    };
    if let Some(frames) = call.recorded() {
        let mut previous = 0;
        let mut terminal = None;
        for (time, payload) in frames {
            if terminal.is_some() {
                return Err(error());
            }
            call.wait(&mut previous, time).await?;
            if super::originals::restore(&payload)? {
                continue;
            }
            terminal = Some(
                serde_json::from_value::<Result<SavedTranscription>>(payload)
                    .map_err(|_| error())?,
            );
        }
        if call.cancelled().is_some() {
            if terminal.is_some() {
                return Err(error());
            }
            return call.require_cancellation(&mut previous).await;
        }
        let saved = terminal.ok_or_else(error)?;
        call.finish()?;
        return saved.map(|v| TranscriptionOutcome {
            result: v.result,
            diagnostics: v.diagnostics,
        });
    }
    let result = call
        .originals(transcription_provider::transcribe_live(
            client, target, key, input, install,
        ))
        .await;
    let payload = match &result {
        Ok(value) => json!({"Ok":{"result":value.result,"diagnostics":value.diagnostics}}),
        Err(error) => json!({"Err":error}),
    };
    call.push(without_secret(payload, key))?;
    call.finish()?;
    result
}
