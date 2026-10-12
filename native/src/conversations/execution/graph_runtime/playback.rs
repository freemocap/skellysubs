//! Product playback reads adopted native outputs; it cannot demand synthesis.
use super::*;
use crate::{
    language::source_graph::SourceText,
    model::{SpeechAudioState, SpeechUnavailableReason},
    speech::{alignment::SpeechAudio, graph_audio, synthesis_graph},
};
use rusqlite::OptionalExtension;

fn invalid() -> AppError {
    AppError::new(
        ErrorCode::Conflict,
        "Native speech source or ownership changed.",
    )
}
pub(super) fn identity(db: &Connection, operation: &str) -> Result<(String, String, String)> {
    if let Some(run) = operation.strip_prefix("graph-request:") {
        let engine: String = db.query_row(
            "SELECT engine_id FROM graph_speech_requests WHERE run_id=?1",
            [run],
            |r| r.get(0),
        )?;
        return Ok((engine, run.into(), "audio".into()));
    }
    serde_json::from_str(operation.strip_prefix("graph:").ok_or_else(invalid)?)
        .map_err(|_| invalid())
}
fn current_source(db: &Connection, engine: &str, run: &str) -> Result<SourceText> {
    db.query_row("SELECT m.id,m.text FROM graph_conversation_runs o JOIN turns t ON t.id=o.turn_id JOIN messages m ON m.turn_id=t.id JOIN conversations c ON c.id=t.conversation_id JOIN contacts contact ON contact.id=c.contact_id WHERE o.engine_id=?1 AND o.run_id=?2 AND o.channel IN ('persona_reply','persona_opening','speech') AND m.role='assistant' AND c.archived=0 AND contact.archived=0 AND t.state NOT IN ('cancelled','invalidated') AND NOT EXISTS(SELECT 1 FROM turns child WHERE child.replaces_turn_id=t.id)",params![engine,run],|r|Ok(SourceText{id:r.get(0)?,text:r.get(1)?})).optional()?.ok_or_else(invalid)
}
impl Runtime {
    pub fn speech_stream_execution(
        &self,
        db: &Connection,
        operation: &str,
    ) -> Result<Option<String>> {
        let (engine_id, run, node) = identity(db, operation)?;
        current_source(db, &engine_id, &run)?;
        let engine = self.engines.get(&engine_id).ok_or_else(invalid)?;
        let view = engine.inspect(&run).map_err(error)?;
        if !view
            .artifact
            .definition
            .nodes
            .get(&node)
            .is_some_and(|n| n.operation == synthesis_graph::playback::select_operation())
        {
            return Err(invalid());
        }
        let candidates: Vec<_> = ancestors(&view, &node)?
            .into_iter()
            .filter(|n| {
                view.artifact.definition.nodes[n].operation == synthesis_graph::operation_contract()
            })
            .collect();
        if candidates.len() != 1 {
            return Err(invalid());
        }
        let Some(attempt) = view.attempts.get(&candidates[0]).and_then(|a| a.last()) else {
            return Ok(None);
        };
        if !matches!(
            attempt.state,
            AttemptState::Running | AttemptState::Available | AttemptState::Adopted
        ) {
            return Ok(None);
        }
        graph_audio::producer_id(&InvocationIdentity {
            engine: Some(engine_id),
            artifact: view.artifact_id.into(),
            execution: attempt.execution,
            operation: synthesis_graph::operation_contract(),
        })
        .map(Some)
    }
    pub fn speech_audio(&self, db: &Connection, operation: &str) -> Result<SpeechAudioState> {
        let (engine_id, run, node) = identity(db, operation)?;
        let tx = db.unchecked_transaction()?;
        let source = current_source(&tx, &engine_id, &run)?;
        let engine = self.engines.get(&engine_id).ok_or_else(invalid)?;
        let view = engine.inspect(&run).map_err(error)?;
        if !view
            .artifact
            .definition
            .nodes
            .get(&node)
            .is_some_and(|n| n.operation == synthesis_graph::playback::select_operation())
        {
            return Err(invalid());
        }
        let mut reader =
            graph_store::BorrowedReadStore::new(&tx, self.partition(&engine_id)?).map_err(error)?;
        let outputs = engine
            .read_node_outputs(&run, &node, &mut reader)
            .map_err(error)?;
        let attempt = view.attempts.get(&node).and_then(|a| a.last());
        let attempt_id = attempt.map(|a| {
            format!(
                "graph:{}",
                serde_json::to_string(&(&engine_id, a.id)).unwrap()
            )
        });
        let unavailable = |reason, message: &str, diagnostics| SpeechAudioState::Unavailable {
            operation_id: operation.into(),
            message_id: source.id.clone(),
            reason,
            message: message.into(),
            attempt_id: attempt_id.clone(),
            diagnostics,
        };
        if !engine.run_control(&run).map_err(error)?.active
            || ancestors(&view, &node)?
                .iter()
                .any(|n| view.nodes[n] == Disposition::Cancelled)
        {
            return Ok(unavailable(
                SpeechUnavailableReason::Cancelled,
                "Speech was cancelled.",
                None,
            ));
        }
        let Some(outputs) = outputs else {
            // Dependency facts come directly from the executable's native view.
            // A failed dependency is explained by the same snapshot, not a new
            // scheduler or an inference from a missing receipt.
            let dependencies = ancestors(&view, &node)?;
            let has = |state| dependencies.iter().any(|node| view.nodes[node] == state);
            let status = if has(Disposition::Unknown) {
                Disposition::Unknown
            } else if has(Disposition::Cancelled) {
                Disposition::Cancelled
            } else if has(Disposition::Failed) || has(Disposition::Blocked) {
                Disposition::Failed
            } else if has(Disposition::Unrequested) || has(Disposition::Disabled) {
                Disposition::Unrequested
            } else {
                view.nodes[&node].clone()
            };
            let diagnostics = if matches!(
                status,
                Disposition::Failed | Disposition::Blocked | Disposition::Unknown
            ) {
                let mut responses = Vec::new();
                for dependency in dependencies {
                    if let Some(attempt) = view.attempts.get(&dependency).and_then(|a| a.last())
                        && let Some(evidence) = engine
                            .read_execution_evidence(attempt.execution, &mut reader)
                            .map_err(error)?
                    {
                        responses.push(serde_json::json!({"node":dependency,"execution":attempt.execution,"observations":evidence.observations,"capture_failed":evidence.evidence_failure.is_some(),"complete":evidence.complete}));
                    }
                }
                Some(
                    serde_json::json!({"graph":engine.inspection_snapshot(&run, ExportLimits{bytes:4*1024*1024,attempts:4096}).map_err(error)?,"responses":responses}),
                )
            } else {
                None
            };
            return Ok(match status {
                Disposition::Failed | Disposition::Blocked => unavailable(
                    SpeechUnavailableReason::Failed,
                    "Speech did not complete. Open AI activity for details.",
                    diagnostics,
                ),
                Disposition::Unknown => unavailable(
                    SpeechUnavailableReason::UnknownOutcome,
                    "Speech outcome is unknown. Explicit retry may repeat provider work.",
                    diagnostics,
                ),
                Disposition::Cancelled => unavailable(
                    SpeechUnavailableReason::Cancelled,
                    "Speech was cancelled.",
                    None,
                ),
                Disposition::Disabled | Disposition::Unrequested | Disposition::Skipped => {
                    unavailable(
                        SpeechUnavailableReason::NotRequested,
                        "Speech has not been requested.",
                        None,
                    )
                }
                _ => SpeechAudioState::Pending {
                    operation_id: operation.into(),
                    message_id: source.id,
                },
            });
        };
        let audio = outputs.get("audio").ok_or_else(invalid)?;
        if audio["source"] != serde_json::to_value(&source)? {
            return Err(invalid());
        }
        let receipt: synthesis_graph::Receipt = serde_json::from_value(audio["receipt"].clone())?;
        graph_audio::verify(&tx, &receipt)?;
        let payload = if let Some(cached) = graph_audio::read(&tx, &receipt.id)? {
            Some(SpeechAudio::new(&cached.wav, cached.alignment))
        } else {
            self.audio_delivery
                .peek(&receipt.id)
                .filter(|a| a.message_id == source.id)
                .map(|a| SpeechAudio::new(&a.wav, a.alignment))
        };
        let Some(payload) = payload else {
            return Ok(unavailable(
                SpeechUnavailableReason::Expired,
                "Audio is no longer cached. Request playback to generate it again.",
                None,
            ));
        };
        let attempt = attempt.ok_or_else(invalid)?;
        let turn: String = tx.query_row(
            "SELECT turn_id FROM graph_conversation_runs WHERE engine_id=?1 AND run_id=?2",
            params![engine_id, run],
            |r| r.get(0),
        )?;
        let digest = crate::ai::results::digest(&serde_json::to_vec(&payload)?);
        tx.execute("INSERT INTO graph_audio_deliveries(engine_id,turn_id,node_key,attempt_id,receipt_id,message_id,audio_digest) VALUES(?1,?2,?3,?4,?5,?6,?7) ON CONFLICT(engine_id,attempt_id) DO NOTHING",params![engine_id,turn,node,serde_json::to_string(&attempt.id)?,receipt.id,source.id,digest])?;
        let exact:bool=tx.query_row("SELECT EXISTS(SELECT 1 FROM graph_audio_deliveries WHERE engine_id=?1 AND turn_id=?2 AND node_key=?3 AND attempt_id=?4 AND receipt_id=?5 AND message_id=?6 AND audio_digest=?7)",params![engine_id,turn,node,serde_json::to_string(&attempt.id)?,receipt.id,source.id,digest],|r|r.get(0))?;
        if !exact {
            return Err(invalid());
        }
        tx.commit()?;
        self.audio_delivery.discard(&receipt.id);
        Ok(SpeechAudioState::Ready {
            operation_id: operation.into(),
            attempt_id: attempt_id.ok_or_else(invalid)?,
            message_id: source.id,
            mime: "audio/wav".into(),
            audio_base64: payload.audio_base64,
            alignment: payload.alignment,
        })
    }

    pub fn delivered_speech_owner(
        &self,
        db: &Connection,
        operation: &str,
        attempt: &str,
        audio: &SpeechAudio,
    ) -> Result<crate::speech::recording::owner::RecordingOwner> {
        let (engine, run, node) = identity(db, operation)?;
        let (attempt_engine, attempt): (String, AttemptId) =
            serde_json::from_str(attempt.strip_prefix("graph:").ok_or_else(invalid)?)?;
        if attempt_engine != engine {
            return Err(invalid());
        }
        let tx = db.unchecked_transaction()?;
        let source = current_source(&tx, &engine, &run)?;
        let mut reader =
            graph_store::BorrowedReadStore::new(&tx, self.partition(&engine)?).map_err(error)?;
        let values = self
            .engines
            .get(&engine)
            .ok_or_else(invalid)?
            .read_node_outputs(&run, &node, &mut reader)
            .map_err(error)?
            .ok_or_else(invalid)?;
        if values.get("audio").and_then(|audio| audio.get("source"))
            != Some(&serde_json::to_value(source)?)
        {
            return Err(invalid());
        }
        let view = self
            .engines
            .get(&engine)
            .ok_or_else(invalid)?
            .inspect(&run)
            .map_err(error)?;
        if view.nodes.get(&node) != Some(&Disposition::Adopted)
            || !view
                .attempts
                .get(&node)
                .and_then(|a| a.last())
                .is_some_and(|a| a.id == attempt)
        {
            return Err(invalid());
        }
        let digest = crate::ai::results::digest(&serde_json::to_vec(audio)?);
        let turn: String = tx.query_row(
            "SELECT turn_id FROM graph_conversation_runs WHERE engine_id=?1 AND run_id=?2",
            params![engine, run],
            |r| r.get(0),
        )?;
        let conversation: String = db.query_row("SELECT t.conversation_id FROM graph_audio_deliveries d JOIN turns t ON t.id=d.turn_id JOIN messages m ON m.id=d.message_id WHERE d.engine_id=?1 AND d.turn_id=?2 AND d.node_key=?3 AND d.attempt_id=?4 AND d.audio_digest=?5 AND t.state NOT IN ('cancelled','invalidated') AND NOT EXISTS(SELECT 1 FROM turns child WHERE child.replaces_turn_id=t.id)", params![engine,turn,node,serde_json::to_string(&attempt)?,digest],|r|r.get(0)).optional()?.ok_or_else(invalid)?;
        let owner = crate::speech::recording::owner::RecordingOwner::Conversation(conversation);
        if !owner.available(db)? {
            return Err(invalid());
        }
        Ok(owner)
    }
}
