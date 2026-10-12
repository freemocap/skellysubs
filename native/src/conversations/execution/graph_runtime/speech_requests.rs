//! Explicit speech uses the shared playback artifact and a message-owned run.
use super::*;
use crate::{language::source_graph::SourceText, speech::synthesis_graph};
use rusqlite::OptionalExtension;

pub enum SpeechCommand {
    Begin {
        run: String,
        turn: String,
        message: String,
        conversation: String,
        inputs: Values,
    },
    Cancel {
        engine: String,
        run: String,
        node: Option<String>,
    },
}
impl Runtime {
    pub fn requested_speech(&self, db: &Connection, message: &str) -> Result<Option<String>> {
        let requested: Option<(String,String)> = db.query_row("SELECT engine_id,run_id FROM graph_speech_requests WHERE message_id=?1 ORDER BY rowid DESC LIMIT 1",[message],|r|Ok((r.get(0)?,r.get(1)?))).optional()?;
        if let Some((engine, run)) = requested {
            return Ok(Some(format!(
                "graph:{}",
                serde_json::to_string(&(engine, run, "audio"))?
            )));
        }
        let owner: Option<(String,String)> = db.query_row("SELECT o.engine_id,o.run_id FROM turn_execution_owners o JOIN messages m ON m.turn_id=o.turn_id WHERE m.id=?1 AND o.executor='graph'",[message],|r|Ok((r.get(0)?,r.get(1)?))).optional()?;
        let Some((engine, run)) = owner else {
            return Ok(None);
        };
        let Some(host) = self.engines.get(&engine) else {
            return Ok(None);
        };
        let view = host.inspect(&run).map_err(error)?;
        let node = view
            .artifact
            .definition
            .nodes
            .iter()
            .find(|(_, n)| n.operation == synthesis_graph::playback::select_operation())
            .map(|(n, _)| n);
        node.map(|node| {
            Ok(format!(
                "graph:{}",
                serde_json::to_string(&(&engine, &run, node))?
            ))
        })
        .transpose()
    }

    pub fn prepare_speech_request(
        &self,
        db: &Connection,
        message: &str,
        regenerate: bool,
    ) -> Result<Option<(String, Option<SpeechCommand>)>> {
        let row: Option<(String,String,String,String)> = db.query_row("SELECT t.id,t.conversation_id,m.text,t.context FROM messages m JOIN turns t ON t.id=m.turn_id JOIN turn_execution_owners o ON o.turn_id=t.id JOIN conversations c ON c.id=t.conversation_id JOIN contacts p ON p.id=c.contact_id WHERE m.id=?1 AND m.role='assistant' AND o.executor='graph' AND o.channel IN ('persona_reply','persona_opening') AND c.archived=0 AND p.archived=0 AND t.state NOT IN ('cancelled','invalidated') AND NOT EXISTS(SELECT 1 FROM turns child WHERE child.replaces_turn_id=t.id)",[message],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?,r.get(3)?))).optional()?;
        let Some((turn, conversation, text, raw)) = row else {
            let native: bool = db.query_row("SELECT EXISTS(SELECT 1 FROM messages m JOIN turn_execution_owners o ON o.turn_id=m.turn_id WHERE m.id=?1 AND o.executor='graph')",[message],|r|r.get(0))?;
            if native {
                return Err(AppError::new(
                    ErrorCode::Conflict,
                    "Speech source is unavailable.",
                ));
            }
            return Ok(None);
        };
        if let Some(operation) = self.requested_speech(db, message)?
            && !regenerate
        {
            let (engine, run, node) = playback::identity(db, &operation)?;
            let host = self
                .engines
                .get(&engine)
                .ok_or_else(|| AppError::new(ErrorCode::Conflict, "Speech engine unavailable."))?;
            let view = host.inspect(&run).map_err(error)?;
            let dependencies = ancestors(&view, &node)?;
            if dependencies.iter().all(|n| {
                !matches!(
                    view.nodes[n],
                    Disposition::Failed
                        | Disposition::Unknown
                        | Disposition::Cancelled
                        | Disposition::Unrequested
                        | Disposition::Disabled
                )
            }) {
                if view.nodes[&node] != Disposition::Adopted {
                    return Ok(Some((operation, None)));
                }
                let mut reader = graph_store::BorrowedReadStore::new(db, self.partition(&engine)?)
                    .map_err(error)?;
                if let Some(values) = host
                    .read_node_outputs(&run, &node, &mut reader)
                    .map_err(error)?
                {
                    let receipt = values["audio"]["receipt"]["id"].as_str().ok_or_else(|| {
                        AppError::new(ErrorCode::Storage, "Speech receipt missing.")
                    })?;
                    if crate::speech::graph_audio::read(db, receipt)?.is_some()
                        || self.audio_delivery.contains(receipt)
                    {
                        return Ok(Some((operation, None)));
                    }
                }
            }
        }
        if crate::ai::connections::configuration::config(db)?.paused {
            return Err(AppError::new(
                ErrorCode::AdmissionHeld,
                "AI execution is paused. Speech was not queued.",
            ));
        }
        super::super::admit_network_work(db, self, 1)?;
        let captured: serde_json::Value = serde_json::from_str(&raw)?;
        let language = serde_json::from_value(captured["languageContext"].clone())?;
        let target = crate::ai::connections::speech_routing::resolve(
            db,
            crate::ai::connections::access::Capability::Speech,
            &language,
        )?;
        let input = super::super::speech_input(
            &target,
            text.clone(),
            captured["speechVoice"].as_str().unwrap_or_default().into(),
            &language,
        )?;
        let install = db.query_row("SELECT id FROM learner LIMIT 1", [], |r| r.get(0))?;
        let mut inputs = synthesis_graph::capture(
            SourceText {
                id: message.into(),
                text,
            },
            synthesis_graph::Settings {
                target,
                install_id: install,
                language_tag: input.language_tag,
                language: input.language,
                voice: input.voice,
            },
        )
        .map_err(error)?;
        inputs.insert("regenerate".into(), serde_json::json!(regenerate));
        let run = uuid::Uuid::new_v4().to_string();
        let operation = format!("graph-request:{run}");
        Ok(Some((
            operation,
            Some(SpeechCommand::Begin {
                run,
                turn,
                message: message.into(),
                conversation,
                inputs,
            }),
        )))
    }

    pub fn prepare_speech_cancel(
        &self,
        db: &Connection,
        operation: &str,
    ) -> Result<(String, SpeechCommand)> {
        let (engine, run, node) = playback::identity(db, operation)?;
        let conversation = db.query_row("SELECT t.conversation_id FROM graph_conversation_runs o JOIN turns t ON t.id=o.turn_id WHERE o.engine_id=?1 AND o.run_id=?2",params![engine,run],|r|r.get(0))?;
        let view = self
            .engines
            .get(&engine)
            .ok_or_else(|| AppError::new(ErrorCode::Conflict, "Speech engine unavailable."))?
            .inspect(&run)
            .map_err(error)?;
        if !view
            .artifact
            .definition
            .nodes
            .get(&node)
            .is_some_and(|n| n.operation == synthesis_graph::playback::select_operation())
        {
            return Err(AppError::new(
                ErrorCode::Conflict,
                "Invalid speech operation.",
            ));
        }
        let synthesis = ancestors(&view, &node)?
            .into_iter()
            .find(|n| {
                view.artifact.definition.nodes[n].operation == synthesis_graph::operation_contract()
            })
            .ok_or_else(|| AppError::new(ErrorCode::Conflict, "Missing speech producer."))?;
        let auxiliary: bool = db.query_row(
            "SELECT EXISTS(SELECT 1 FROM graph_speech_requests WHERE run_id=?1)",
            [&run],
            |r| r.get(0),
        )?;
        Ok((
            conversation,
            SpeechCommand::Cancel {
                engine,
                run,
                node: if auxiliary { None } else { Some(synthesis) },
            },
        ))
    }

    pub fn speech_request_view(
        &self,
        db: &Connection,
        turn: &str,
    ) -> Result<Option<crate::model::SpeechRequestView>> {
        let message: Option<String> = db
            .query_row(
                "SELECT id FROM messages WHERE turn_id=?1 AND role='assistant'",
                [turn],
                |r| r.get(0),
            )
            .optional()?;
        let Some(message) = message else {
            return Ok(None);
        };
        let Some(operation) = self.requested_speech(db, &message)? else {
            return Ok(None);
        };
        let (engine, run, node) = playback::identity(db, &operation)?;
        let Some(host) = self.engines.get(&engine) else {
            return Ok(None);
        };
        let view = host.inspect(&run).map_err(error)?;
        // Playback discovery lists requests, while native inspection continues to
        // expose every defined node, including speech that was never requested.
        if ancestors(&view, &node)?.iter().any(|n| {
            matches!(
                view.nodes[n],
                Disposition::Unrequested | Disposition::Disabled
            )
        }) {
            return Ok(None);
        }
        let state = message_view::state(&view.nodes[&node]).unwrap_or_else(|| "unrequested".into());
        Ok(Some(crate::model::SpeechRequestView {
            id: operation,
            state,
            source_message_id: message,
        }))
    }

    pub fn apply_speech_command(
        &mut self,
        tx: Transaction<'_>,
        command: SpeechCommand,
    ) -> Result<()> {
        match command {
            SpeechCommand::Begin {
                run,
                turn,
                message,
                conversation,
                inputs,
            } => {
                let graph = self.partner.playback.clone();
                let partition = Partition {
                    owner: crate::ai::graph_store::Owner::Conversation(conversation.clone()),
                    catalog: graph_store::catalog_id([graph.identity()]).map_err(error)?,
                };
                let existing: Option<String> = tx
                    .query_row(
                        "SELECT id FROM graph_engines WHERE conversation_id=?1 AND catalog=?2",
                        params![conversation, partition.catalog],
                        |r| r.get(0),
                    )
                    .optional()?;
                let event = Event::Begin {
                    run: run.clone(),
                    artifact: graph.identity().into(),
                    inputs,
                    scope: format!("speech:{run}"),
                    policy: BTreeMap::from([("lookup".into(), Activation::Automatic)]),
                };
                let mut domain_error = None;
                let callback = |db: &Connection, request: &CommitRequest<'_>| {
                    let CommitIntent::Begin { authority, .. } = &request.intent else {
                        return Err(Fault {
                            code: "speech_begin_required".into(),
                            path: "owner".into(),
                        });
                    };
                    db.execute(
                        "INSERT INTO graph_speech_requests VALUES(?1,?2,?3,?4,?5,?6)",
                        params![
                            run,
                            request.next.stamp().engine,
                            authority.artifact,
                            turn,
                            message,
                            authority.scope
                        ],
                    )
                    .map(|_| ())
                    .map_err(|cause| {
                        domain_error = Some(AppError::from(cause));
                        Fault {
                            code: "speech_owner_write_failed".into(),
                            path: "owner".into(),
                        }
                    })
                };
                let mut adapter = TransactionStore::new(
                    tx,
                    partition.clone(),
                    limits().checkpoint.bytes,
                    callback,
                );
                let result = if let Some(engine) = existing {
                    self.engines
                        .get_mut(&engine)
                        .ok_or_else(|| Fault {
                            code: "speech_engine_unavailable".into(),
                            path: "engine".into(),
                        })
                        .and_then(|engine| engine.apply(event, &mut adapter))
                        .map(|_| None)
                } else {
                    DurableEngine::create_with_run([graph], limits(), event, &mut adapter).map(Some)
                };
                drop(adapter);
                if let Some(engine) =
                    result.map_err(|fault| domain_error.unwrap_or_else(|| error(fault)))?
                {
                    self.install(partition, engine);
                }
            }
            SpeechCommand::Cancel { engine, run, node } => {
                let partition = self.partition(&engine)?;
                let mut adapter = TransactionStore::new(
                    tx,
                    partition,
                    limits().checkpoint.bytes,
                    |_: &Connection, _: &CommitRequest<'_>| Ok(()),
                );
                self.engines
                    .get_mut(&engine)
                    .ok_or_else(|| {
                        AppError::new(ErrorCode::Conflict, "Speech engine unavailable.")
                    })?
                    .apply(Event::Cancel { run, node }, &mut adapter)
                    .map_err(error)?;
            }
        }
        Ok(())
    }
}
