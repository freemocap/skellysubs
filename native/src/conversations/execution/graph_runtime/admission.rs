use super::super::{context, partner_graph};
use super::*;
use rusqlite::OptionalExtension;

pub enum Admission {
    Coach,
    Partner(context::Kind),
}

impl Runtime {
    /// Consumes the command transaction only after its finalized inputs and
    /// action receipt have been staged. Native Begin is its single commit.
    pub fn admit(&mut self, tx: Transaction<'_>, turn: &str, admission: Admission) -> Result<()> {
        let (conversation,raw,install):(String,String,String)=tx.query_row(
            "SELECT t.conversation_id,t.context,m.id FROM turns t CROSS JOIN learner m WHERE t.id=?1",
            [turn],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?)))?;
        let captured: serde_json::Value = serde_json::from_str(&raw)?;
        let (graph, inputs, policy, channel) = match admission {
            Admission::Coach => (
                self.graph.clone(),
                coach_graph::capture(
                    serde_json::from_value(captured["messages"].clone())?,
                    serde_json::from_value(captured["sourceIds"].clone())?,
                    &serde_json::from_value(captured["target"].clone())?,
                    &install,
                )
                .map_err(error)?,
                BTreeMap::new(),
                "coach",
            ),
            Admission::Partner(kind) => {
                let learner = if matches!(kind, context::Kind::Reply) {
                    Some(tx.query_row(
                        "SELECT id,text FROM messages WHERE turn_id=?1 AND role='user'",
                        [turn],
                        |r| {
                            Ok(crate::language::source_graph::SourceText {
                                id: r.get(0)?,
                                text: r.get(1)?,
                            })
                        },
                    )?)
                } else {
                    None
                };
                let graph = if matches!(kind, context::Kind::Reply) {
                    self.partner.reply.clone()
                } else {
                    self.partner.opening.clone()
                };
                let inputs = partner_graph::captured_turn::inputs(
                    kind,
                    &captured,
                    learner,
                    uuid::Uuid::new_v4().to_string(),
                    install,
                )
                .map_err(error)?;
                let policy =
                    partner_graph::captured_turn::policy(kind, &captured).map_err(error)?;
                (
                    graph,
                    inputs,
                    policy,
                    if matches!(kind, context::Kind::Reply) {
                        "persona_reply"
                    } else {
                        "persona_opening"
                    },
                )
            }
        };
        super::super::admit_network_work(&tx, self, budget::initial(graph.artifact(), &policy)?)?;
        let begin = Event::Begin {
            run: turn.into(),
            artifact: graph.identity().into(),
            inputs,
            scope: format!("{channel}-turn:{turn}"),
            policy,
        };
        let partition = Partition {
            owner: crate::ai::graph_store::Owner::Conversation(conversation.clone()),
            catalog: if channel == "coach" {
                self.catalog.clone()
            } else {
                graph_store::catalog_id([graph.identity()]).map_err(error)?
            },
        };
        let existing: Option<String> = tx
            .query_row(
                "SELECT id FROM graph_engines WHERE conversation_id=?1 AND catalog=?2",
                params![conversation, partition.catalog],
                |r| r.get(0),
            )
            .optional()?;
        let mut domain_error = None;
        let callback = |db: &Connection, request: &CommitRequest<'_>| {
            let staged = (|| -> Result<()> {
                let CommitIntent::Begin { authority, .. } = &request.intent else {
                    return Err(AppError::new(
                        ErrorCode::Internal,
                        "Expected graph admission.",
                    ));
                };
                db.execute(
                    "INSERT INTO turn_execution_owners VALUES(?1,'graph',?5,?2,?3,?4)",
                    params![
                        turn,
                        request.next.stamp().engine,
                        authority.run,
                        authority.artifact,
                        channel
                    ],
                )?;
                graph_publication::declare_reply(
                    db,
                    request,
                    coach_graph::REPLY,
                    coach_graph::TEXT,
                )?;
                Ok(())
            })();
            staged.map_err(|cause| {
                domain_error = Some(cause);
                Fault {
                    code: "graph_admission_rejected".into(),
                    path: "owner".into(),
                }
            })
        };
        let mut adapter =
            TransactionStore::new(tx, partition.clone(), limits().checkpoint.bytes, callback);
        let result = if let Some(id) = existing {
            self.engines
                .get_mut(&id)
                .ok_or_else(|| Fault {
                    code: "graph_engine_unavailable".into(),
                    path: "engine".into(),
                })
                .and_then(|engine| engine.apply(begin, &mut adapter))
                .map(|_| ())
        } else {
            DurableEngine::create_with_run([graph], limits(), begin, &mut adapter).map(|engine| {
                self.install(partition, engine);
            })
        };
        drop(adapter);
        result.map_err(|fault| domain_error.unwrap_or_else(|| error(fault)))
    }

    pub fn recover(&mut self, db: &mut Connection) -> Result<()> {
        let partitions = db
            .prepare("SELECT conversation_id,catalog FROM graph_engines ORDER BY rowid")?
            .query_map([], |r| {
                Ok(Partition {
                    owner: crate::ai::graph_store::Owner::Conversation(r.get(0)?),
                    catalog: r.get(1)?,
                })
            })?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        for partition in partitions {
            let Some(graphs) = self.catalogs.get(&partition.catalog).cloned() else {
                continue; // Retained inspection does not require executable capabilities.
            };
            let conversation = partition.owner.key().to_owned();
            let checkpoint = {
                let reader = graph_store::ReadStore::new(db, partition.clone()).map_err(error)?;
                reader
                    .checkpoint(limits().checkpoint)
                    .map_err(error)?
                    .ok_or_else(|| AppError::new(ErrorCode::Storage, "Graph checkpoint missing."))?
            };
            let callback = |db: &Connection, _: &CommitRequest<'_>| {
                db.execute("UPDATE turns SET paused=1 WHERE id IN (SELECT turn_id FROM turn_execution_owners WHERE executor='graph' AND engine_id IN (SELECT id FROM graph_engines WHERE conversation_id=?1 AND catalog=?2))",params![conversation, partition.catalog])
                    .map(|_|()).map_err(|_|Fault{code:"graph_recovery_failed".into(),path:"owner".into()})
            };
            let mut adapter = TransactionStore::new(
                db.transaction()?,
                partition.clone(),
                limits().checkpoint.bytes,
                callback,
            );
            let engine = DurableEngine::recover(checkpoint, graphs, limits(), &mut adapter)
                .map_err(error)?;
            drop(adapter);
            let runs = db
                .prepare("SELECT run_id FROM graph_conversation_runs WHERE engine_id=?1")?
                .query_map([&engine.stamp().engine], |r| r.get::<_, String>(0))?
                .collect::<rusqlite::Result<Vec<_>>>()?;
            for run in runs {
                if status::partner(db, &engine, &run)? {
                    continue;
                }
                let view = engine.inspect(&run).map_err(error)?;
                let state = if view
                    .nodes
                    .values()
                    .any(|s| matches!(s, Disposition::Unknown))
                {
                    Some("unknown")
                } else if view
                    .nodes
                    .values()
                    .any(|s| matches!(s, Disposition::Failed))
                {
                    Some("failed")
                } else {
                    None
                };
                if let Some(state) = state {
                    db.execute("UPDATE turns SET state=?2 WHERE id=?1 AND state IN ('pending','assisting')",params![run,state])?;
                }
            }
            self.install(partition, engine);
        }
        Ok(())
    }
}
