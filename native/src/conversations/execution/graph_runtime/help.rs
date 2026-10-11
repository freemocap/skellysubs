//! Explicit product requests bind to native operations and exact message inputs.
//! Action receipts and native demand/retry commit in one owner transaction.
use super::*;
use crate::language::source_graph::SourceText;
use rusqlite::OptionalExtension;

pub struct HelpRequest {
    pub conversation: String,
    pub operation: String,
    pub(super) engine: String,
    pub(super) run: String,
    pub(super) node: String,
    pub(super) source: SourceText,
    pub(super) retry: bool,
    pub(super) fresh: Option<super::helper_requests::Fresh>,
}
fn rejected() -> AppError {
    AppError::new(
        ErrorCode::Conflict,
        "This assistance is unavailable for the current message.",
    )
}
impl Runtime {
    pub fn resolve_gloss_retry(
        &self,
        db: &Connection,
        operation: &str,
    ) -> Result<Option<HelpRequest>> {
        let Some(encoded) = operation.strip_prefix("graph:") else {
            return Ok(None);
        };
        let (engine_id, run, node): (String, String, String) =
            serde_json::from_str(encoded).map_err(|_| rejected())?;
        let engine = self.engines.get(&engine_id).ok_or_else(rejected)?;
        let view = engine.inspect(&run).map_err(error)?;
        let contract = crate::language::gloss_graph::operation_contract();
        if view
            .artifact
            .definition
            .nodes
            .get(&node)
            .is_none_or(|definition| definition.operation != contract)
        {
            return Err(rejected());
        }
        let mut reader =
            graph_store::BorrowedReadStore::new(db, self.partition(&engine_id)?).map_err(error)?;
        let source: SourceText = serde_json::from_value(
            engine
                .read_node_input(&run, &node, "source", &mut reader)
                .map_err(error)?
                .ok_or_else(rejected)?,
        )?;
        let owner: Option<(String,String)> = db.query_row("SELECT t.conversation_id,t.id FROM graph_conversation_runs o JOIN turns t ON t.id=o.turn_id JOIN messages m ON m.turn_id=t.id JOIN conversations c ON c.id=t.conversation_id JOIN contacts p ON p.id=c.contact_id WHERE o.engine_id=?1 AND o.run_id=?2 AND m.id=?3 AND m.text=?4 AND c.archived=0 AND p.archived=0 AND t.state NOT IN ('cancelled','invalidated') AND NOT EXISTS(SELECT 1 FROM turns child WHERE child.replaces_turn_id=t.id)",params![engine_id,run,source.id,source.text],|r|Ok((r.get(0)?,r.get(1)?))).optional()?;
        let (conversation, turn) = owner.ok_or_else(rejected)?;
        let fresh = if view.nodes[&node] == Disposition::Adopted {
            let saved: Option<String> = db.query_row("SELECT json_extract(t.context,CASE m.role WHEN 'user' THEN '$.userWordGloss' ELSE '$.wordGloss' END) FROM messages m JOIN turns t ON t.id=m.turn_id WHERE m.id=?1",[&source.id],|r|r.get(0))?;
            let saved: Option<crate::model::WordGlossView> =
                saved.map(|v| serde_json::from_str(&v)).transpose()?;
            if saved.is_some_and(|v| {
                v.coverage != crate::model::GlossCoverage::Partial
                    && v.segments
                        .iter()
                        .any(|s| s.kind == crate::model::GlossSegmentKind::Gloss)
            }) {
                return Err(rejected());
            }

            let mut inputs = Values::new();
            for port in crate::language::gloss_graph::inputs().keys() {
                inputs.insert(
                    port.clone(),
                    engine
                        .read_node_input(&run, &node, port, &mut reader)
                        .map_err(error)?
                        .ok_or_else(rejected)?,
                );
            }
            let current: Option<String> = db.query_row("SELECT run_id FROM graph_helper_requests WHERE message_id=?1 AND operation=?2 ORDER BY rowid DESC LIMIT 1",params![source.id,serde_json::to_string(&contract)?],|r|r.get(0)).optional()?;
            if current.as_ref().is_some_and(|latest| latest != &run) {
                return Err(rejected());
            }
            super::super::admit_network_work(db, self, 1)?;
            Some(super::helper_requests::Fresh {
                run: uuid::Uuid::new_v4().to_string(),
                turn,
                conversation: conversation.clone(),
                source: source.clone(),
                inputs,
                operation: contract.clone(),
            })
        } else if matches!(
            view.nodes[&node],
            Disposition::Failed | Disposition::Unknown | Disposition::Blocked
        ) {
            None
        } else {
            return Err(rejected());
        };
        let request = HelpRequest {
            conversation,
            operation: fresh.as_ref().map_or_else(
                || operation.into(),
                |request| format!("graph-helper:{}", request.run),
            ),
            engine: engine_id,
            run,
            node,
            source,
            retry: true,
            fresh,
        };
        Ok(Some(request))
    }

    pub fn resolve_help(
        &self,
        db: &Connection,
        message: &str,
        operation: &Contract,
        retry: bool,
    ) -> Result<Option<HelpRequest>> {
        let owner:Option<(String,String,String,String,String,String)> = db.query_row("SELECT o.executor,o.engine_id,o.run_id,m.conversation_id,m.text,t.id FROM messages m JOIN turns t ON t.id=m.turn_id JOIN turn_execution_owners o ON o.turn_id=t.id JOIN conversations c ON c.id=t.conversation_id JOIN contacts contact ON contact.id=c.contact_id WHERE m.id=?1 AND o.executor='graph' AND o.channel IN ('persona_reply','persona_opening') AND c.archived=0 AND contact.archived=0 AND t.state NOT IN ('cancelled','invalidated') AND NOT EXISTS(SELECT 1 FROM turns child WHERE child.replaces_turn_id=t.id)",[message],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?,r.get(3)?,r.get(4)?,r.get(5)?))).optional()?;
        let Some((_, mut engine_id, mut run, conversation, text, _)) = owner else {
            if db.query_row("SELECT EXISTS(SELECT 1 FROM messages m JOIN turn_execution_owners o ON o.turn_id=m.turn_id WHERE m.id=?1 AND o.executor='graph')",[message],|r|r.get::<_,bool>(0))? { return Err(rejected()); }
            return Ok(None);
        };
        let latest:Option<(String,String)>=db.query_row("SELECT engine_id,run_id FROM graph_helper_requests WHERE message_id=?1 AND operation=?2 ORDER BY rowid DESC LIMIT 1",params![message,serde_json::to_string(operation)?],|r|Ok((r.get(0)?,r.get(1)?))).optional()?;
        if let Some((engine, latest_run)) = latest {
            engine_id = engine;
            run = latest_run;
        }
        let engine = self.engines.get(&engine_id).ok_or_else(rejected)?;
        let view = engine.inspect(&run).map_err(error)?;
        let source = SourceText {
            id: message.into(),
            text,
        };
        let mut reader =
            graph_store::BorrowedReadStore::new(db, self.partition(&engine_id)?).map_err(error)?;
        let mut matching = Vec::new();
        for (node, definition) in &view.artifact.definition.nodes {
            if &definition.operation != operation {
                continue;
            }
            let resolved = engine
                .read_node_input(&run, node, "source", &mut reader)
                .map_err(error)?;
            let matches = if let Some(value) = resolved {
                serde_json::from_value::<SourceText>(value)? == source
            } else if let Some(Source::Output {
                node: binding,
                port,
            }) = definition.inputs.get("source")
            {
                // A reply is published before its local source-binding node runs.
                // Its already-resolved ID/text inputs establish the same exact
                // source; demand still waits for the graph to adopt that node.
                let producer = &view.artifact.definition.nodes[binding];
                producer.operation == super::super::reply_reading_graph::source_binding_contract()
                    && port == "source"
                    && engine
                        .read_node_input(&run, binding, "id", &mut reader)
                        .map_err(error)?
                        .as_ref()
                        .and_then(serde_json::Value::as_str)
                        == Some(source.id.as_str())
                    && engine
                        .read_node_input(&run, binding, "text", &mut reader)
                        .map_err(error)?
                        .as_ref()
                        .and_then(serde_json::Value::as_str)
                        == Some(source.text.as_str())
            } else {
                false
            };
            if matches {
                matching.push(node.clone());
            }
        }
        if matching.len() != 1 {
            return Err(rejected());
        }
        let node = matching.remove(0);
        Ok(Some(HelpRequest {
            conversation,
            operation: format!(
                "graph:{}",
                serde_json::to_string(&(&engine_id, &run, &node))?
            ),
            engine: engine_id,
            run,
            node,
            source,
            retry,
            fresh: None,
        }))
    }

    pub fn request_help(&mut self, tx: Transaction<'_>, request: HelpRequest) -> Result<()> {
        if let Some(fresh) = request.fresh {
            return self.begin_fresh_helper(tx, fresh);
        }
        let current:bool=tx.query_row("SELECT EXISTS(SELECT 1 FROM messages m JOIN turns t ON t.id=m.turn_id JOIN graph_conversation_runs o ON o.turn_id=t.id WHERE m.id=?1 AND m.text=?2 AND o.engine_id=?3 AND o.run_id=?4 AND t.state NOT IN ('cancelled','invalidated') AND NOT EXISTS(SELECT 1 FROM turns child WHERE child.replaces_turn_id=t.id))",params![request.source.id,request.source.text,request.engine,request.run],|r|r.get(0))?;
        if !current {
            return Err(rejected());
        }
        let engine = self.engines.get(&request.engine).ok_or_else(rejected)?;
        let view = engine.inspect(&request.run).map_err(error)?;
        let disposition = &view.nodes[&request.node];
        let event = match disposition {
            Disposition::Failed | Disposition::Unknown if request.retry => Some(Event::Retry {
                run: request.run.clone(),
                node: request.node.clone(),
            }),
            Disposition::Blocked if request.retry => {
                let failed: Vec<_> = ancestors(&view, &request.node)?
                    .into_iter()
                    .filter(|node| {
                        matches!(view.nodes[node], Disposition::Failed | Disposition::Unknown)
                    })
                    .collect();
                match failed.as_slice() {
                    [node] => Some(Event::Retry {
                        run: request.run.clone(),
                        node: node.clone(),
                    }),
                    _ => return Err(rejected()),
                }
            }
            Disposition::Disabled | Disposition::Cancelled => return Err(rejected()),
            Disposition::Unrequested | Disposition::Waiting => {
                if request.retry {
                    return Err(AppError::new(
                        ErrorCode::Conflict,
                        "Request this assistance before retrying it.",
                    ));
                }
                let unrequested: Vec<_> = ancestors(&view, &request.node)?
                    .into_iter()
                    .filter(|node| view.nodes[node] == Disposition::Unrequested)
                    .collect();
                match unrequested.as_slice() {
                    [] => None,
                    [node] => Some(Event::Demand {
                        run: request.run.clone(),
                        node: node.clone(),
                    }),
                    _ => {
                        return Err(AppError::new(
                            ErrorCode::Conflict,
                            "This assistance requires multiple independent requests.",
                        ));
                    }
                }
            }
            _ => None,
        };
        let Some(event) = event else {
            tx.commit()?;
            return Ok(());
        };
        let target = match &event {
            Event::Demand { node, .. } | Event::Retry { node, .. } => node,
            _ => unreachable!(),
        };
        let providers = budget::requested(&view, target)?;
        super::super::admit_network_work(&tx, self, providers)?;
        let attempts: usize = view
            .attempts
            .iter()
            .filter(|(node, _)| {
                view.artifact.operations[&view.artifact.definition.nodes[*node].operation].resource
                    == Resource::Provider
            })
            .map(|(_, attempts)| attempts.len())
            .sum();
        if attempts as i64 + providers > super::super::TURN_ATTEMPT_LIMIT {
            return Err(super::super::budget_error(
                "This turn reached its assistance attempt budget.",
            ));
        }
        tx.execute(
            "UPDATE turns SET state='assisting' WHERE id=?1 AND state='succeeded'",
            [&request.run],
        )?;
        let partition = self.partition(&request.engine)?;
        let mut adapter = TransactionStore::new(
            tx,
            partition,
            limits().checkpoint.bytes,
            |_: &Connection, _: &CommitRequest<'_>| Ok(()),
        );
        self.engines
            .get_mut(&request.engine)
            .ok_or_else(rejected)?
            .apply(event, &mut adapter)
            .map_err(error)?;
        Ok(())
    }
}
