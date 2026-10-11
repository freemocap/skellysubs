use super::{Call, begin, error};
use crate::{
    ai::transport::{
        grouped,
        provider::{self, Completion},
        text_request::TextRequest,
    },
    model::Result,
};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

#[derive(Serialize, Deserialize)]
#[serde(tag = "type", deny_unknown_fields)]
enum TextFrame {
    Delta {
        index: usize,
        text: String,
    },
    Item {
        index: usize,
        result: Result<Completion>,
    },
    End {
        result: Result<()>,
    },
}
fn semantic(dispatch: &TextRequest, output: provider::RequestOutput<'_>) -> Result<Value> {
    Ok(json!({"route":dispatch.route, "payload":provider::dispatch_payload(dispatch, output)?}))
}
fn push(call: &Call, frame: TextFrame, key: &str) -> Result<()> {
    call.push(super::without_secret(serde_json::to_value(frame)?, key))
}
pub(in crate::ai::transport) async fn request(
    client: &reqwest::Client,
    key: &str,
    dispatches: &[TextRequest],
    outputs: &[provider::RequestOutput<'_>],
    deltas: bool,
    mut publish: impl FnMut(usize, Result<Completion>) -> Result<()>,
    mut on_delta: impl FnMut(usize, &str),
) -> Result<()> {
    if dispatches.len() != outputs.len() {
        return Err(error());
    }
    let semantics: Result<Vec<_>> = dispatches
        .iter()
        .zip(outputs)
        .map(|(d, o)| semantic(d, *o))
        .collect();
    let Some(call) = begin("text", json!({"requests":semantics?, "deltas":deltas}))? else {
        return grouped::request_streaming_live(
            client, key, dispatches, outputs, deltas, publish, on_delta,
        )
        .await;
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
                TextFrame::Delta { index, text } => {
                    if index >= dispatches.len() {
                        return Err(error());
                    }
                    on_delta(index, &text);
                }
                TextFrame::Item { index, result } => {
                    if index >= dispatches.len() {
                        return Err(error());
                    }
                    publish(index, result)?;
                }
                TextFrame::End { result } => terminal = Some(result),
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
        return result;
    }
    // Delta callback cannot return an error; capture failure is surfaced at end.
    let delta_error = std::sync::Mutex::new(None);
    let result = call
        .originals(grouped::request_streaming_live(
            client,
            key,
            dispatches,
            outputs,
            deltas,
            |index, result| {
                push(
                    &call,
                    TextFrame::Item {
                        index,
                        result: result.clone(),
                    },
                    key,
                )?;
                publish(index, result)
            },
            |index, text| {
                if let Err(error) = push(
                    &call,
                    TextFrame::Delta {
                        index,
                        text: text.into(),
                    },
                    key,
                ) {
                    *delta_error.lock().unwrap_or_else(|e| e.into_inner()) = Some(error);
                }
                on_delta(index, text);
            },
        ))
        .await;
    if let Some(error) = delta_error.into_inner().unwrap_or_else(|e| e.into_inner()) {
        return Err(error);
    }
    push(
        &call,
        TextFrame::End {
            result: result.clone(),
        },
        key,
    )?;
    call.finish()?;
    result
}

pub(in crate::ai::transport) async fn protocol(
    client: &reqwest::Client,
    key: &str,
    dispatch: &TextRequest,
) -> Result<bool> {
    let Some(call) = begin(
        "protocol",
        semantic(dispatch, provider::RequestOutput::Prose)?,
    )?
    else {
        return grouped::supports_deltas_live(client, key, dispatch).await;
    };
    if let Some(frames) = call.recorded() {
        if call.cancelled().is_some() {
            if !frames.is_empty() {
                return Err(error());
            }
            return call.require_cancellation(&mut 0).await;
        }
        if frames.len() != 1 {
            return Err(error());
        }
        let mut previous = 0;
        call.wait(&mut previous, frames[0].0).await?;
        let result: Result<bool> =
            serde_json::from_value(frames[0].1.clone()).map_err(|_| error())?;
        call.finish()?;
        return result;
    }
    let result = grouped::supports_deltas_live(client, key, dispatch).await;
    call.push(super::without_secret(serde_json::to_value(&result)?, key))?;
    call.finish()?;
    result
}
