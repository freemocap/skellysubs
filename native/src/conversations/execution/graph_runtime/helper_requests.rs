//! Fresh requests reuse the same operation and publication adapter. A completed
//! result stays immutable; each explicit regeneration has a separate run/scope.
use super::*;
use crate::language::source_graph::SourceText;
use rusqlite::OptionalExtension;

pub(super) struct Fresh {
    pub run: String,
    pub turn: String,
    pub conversation: String,
    pub source: SourceText,
    pub inputs: Values,
    pub operation: Contract,
}
impl Runtime {
    pub(super) fn begin_fresh_helper(&mut self, tx: Transaction<'_>, request: Fresh) -> Result<()> {
        let graph = [self.partner.gloss.clone(), self.partner.feedback.clone()]
            .into_iter()
            .find(|g| {
                g.artifact().definition.nodes.len() == 1
                    && g.artifact()
                        .definition
                        .nodes
                        .values()
                        .any(|n| n.operation == request.operation)
            })
            .ok_or_else(|| error(fault()))?;
        let node = graph
            .artifact()
            .definition
            .nodes
            .keys()
            .next()
            .ok_or_else(|| error(fault()))?
            .clone();
        let note = if request.operation
            == crate::learning::coaching::feedback_graph::operation_contract()
        {
            crate::learning::coaching::feedback_graph::decode(&request.inputs)
                .map_err(error)?
                .context
                .feedback_context
        } else {
            None
        };
        let partition = Partition {
            owner: graph_store::Owner::Conversation(request.conversation.clone()),
            catalog: graph_store::catalog_id([graph.identity()]).map_err(error)?,
        };
        let existing: Option<String> = tx
            .query_row(
                "SELECT id FROM graph_engines WHERE conversation_id=?1 AND catalog=?2",
                params![request.conversation, partition.catalog],
                |r| r.get(0),
            )
            .optional()?;
        let operation = serde_json::to_string(&request.operation)?;
        let event = Event::Begin {
            run: request.run.clone(),
            artifact: graph.identity().into(),
            inputs: request.inputs,
            scope: format!("helper:{}", request.run),
            policy: BTreeMap::from([(node, Activation::Automatic)]),
        };
        let mut domain_error = None;
        let callback = |db: &Connection, commit: &CommitRequest<'_>| {
            let CommitIntent::Begin { authority, .. } = &commit.intent else {
                return Err(fault());
            };
            let result: Result<()> = (|| {
                let current: bool = db.query_row("SELECT EXISTS(SELECT 1 FROM messages m JOIN turns t ON t.id=m.turn_id JOIN conversations c ON c.id=t.conversation_id JOIN contacts p ON p.id=c.contact_id WHERE m.id=?1 AND m.text=?2 AND t.id=?3 AND t.conversation_id=?4 AND c.archived=0 AND p.archived=0 AND t.state NOT IN ('cancelled','invalidated') AND NOT EXISTS(SELECT 1 FROM turns child WHERE child.replaces_turn_id=t.id))",params![request.source.id,request.source.text,request.turn,request.conversation],|r|r.get(0))?;
                if !current {
                    return Err(AppError::new(ErrorCode::Conflict, "Helper source changed."));
                }
                db.execute(
                    "INSERT INTO graph_helper_requests VALUES(?1,?2,?3,?4,?5,?6,?7)",
                    params![
                        request.run,
                        commit.next.stamp().engine,
                        authority.artifact,
                        request.turn,
                        request.source.id,
                        authority.scope,
                        operation
                    ],
                )?;
                if let Some(note) = &note {
                    db.execute("UPDATE turns SET context=json_set(json_remove(context,'$.coach_feedbackError'),'$.feedbackContext',?2) WHERE id=?1",params![request.turn,note])?;
                }
                Ok(())
            })();
            result.map_err(|cause| {
                domain_error = Some(cause);
                fault()
            })
        };
        let mut adapter =
            TransactionStore::new(tx, partition.clone(), limits().checkpoint.bytes, callback);
        let result = if let Some(engine) = existing {
            self.engines
                .get_mut(&engine)
                .ok_or_else(fault)
                .and_then(|e| e.apply(event, &mut adapter))
                .map(|_| None)
        } else {
            DurableEngine::create_with_run([graph], limits(), event, &mut adapter).map(Some)
        };
        drop(adapter);
        if let Some(engine) =
            result.map_err(|cause| domain_error.unwrap_or_else(|| error(cause)))?
        {
            self.install(partition, engine);
        }
        Ok(())
    }
}
fn fault() -> Fault {
    Fault {
        code: "helper_admission_failed".into(),
        path: "owner".into(),
    }
}
