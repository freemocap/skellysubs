//! Receipt adoption validates durable audio and the current exact source.
//! Playback reads native adopted values, not a second result store.
use super::*;
use crate::{
    ai::graph::Work,
    language::source_graph::SourceText,
    speech::{graph_audio, synthesis_graph},
};

pub(super) fn surface(operation: &crate::ai::graph::Contract, port: &str) -> Option<&'static str> {
    ((operation == &synthesis_graph::operation_contract()
        || operation == &synthesis_graph::playback::select_operation())
        && port == "audio")
        .then_some("audio")
}

pub fn adopt_audio(db: &Connection, request: &CommitRequest<'_>, work: &Work) -> Result<bool> {
    if surface(&work.operation, "audio").is_none() {
        return Ok(false);
    }
    let CommitIntent::Adopt {
        values, execution, ..
    } = &request.intent
    else {
        return Err(rejected());
    };
    let bound = values.get("audio").ok_or_else(rejected)?;
    let source: SourceText = serde_json::from_value(bound["source"].clone())?;
    let captured: SourceText =
        serde_json::from_value(work.inputs.get("source").ok_or_else(rejected)?.clone())?;
    source::adopted_owner(db, request, work, &source, &captured)?;
    let receipt: synthesis_graph::Receipt = serde_json::from_value(bound["receipt"].clone())?;
    graph_audio::verify(db, &receipt)?;
    if work.operation == synthesis_graph::operation_contract()
        && (receipt.engine != request.next.stamp().engine
            || receipt.execution != serde_json::to_string(execution)?)
    {
        return Err(rejected());
    }
    Ok(true)
}
