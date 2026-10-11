//! Product status fields are projections of native nodes and exact resolved
//! sources. This does not create operations, topology or scheduling rules.
use super::*;
use crate::{
    language::{gloss_graph, source_graph::SourceText, translation_graph},
    learning::coaching::{assessment_graph, feedback_graph, support_graph},
    model::ChatMessage,
};
use rusqlite::OptionalExtension;

pub(super) fn state(disposition: &Disposition) -> Option<String> {
    Some(
        match disposition {
            Disposition::Disabled | Disposition::Unrequested | Disposition::Skipped => return None,
            Disposition::Adopted => "succeeded",
            Disposition::Ready | Disposition::Prepared => "ready",
            Disposition::Running | Disposition::Available => "running",
            Disposition::Waiting => "waiting_dependencies",
            Disposition::Paused => "held",
            // Admission capacity is queued work, not a learner pause.
            Disposition::Held => "ready",
            Disposition::Unknown => "unknown",
            Disposition::Failed | Disposition::Blocked => "failed",
            Disposition::Cancelled => "cancelled",
        }
        .into(),
    )
}
#[derive(Clone)]
pub struct SourceStatus {
    pub operation: Contract,
    pub state: Option<String>,
    pub error: Option<String>,
    pub attempt: Option<String>,
    pub node_id: String,
}
type SourceStatuses = BTreeMap<(String, String), Vec<(SourceText, SourceStatus)>>;
impl Runtime {
    pub fn source_status(
        &self,
        db: &Connection,
        turn: &str,
        expected: &SourceText,
    ) -> Result<Vec<SourceStatus>> {
        self.source_status_with_cache(db, turn, expected, &mut SourceStatuses::new())
    }

    fn source_status_with_cache(
        &self,
        db: &Connection,
        turn: &str,
        expected: &SourceText,
        statuses_by_run: &mut SourceStatuses,
    ) -> Result<Vec<SourceStatus>> {
        let mut statuses = Vec::new();
        let owner:Option<(String,String)> = db.query_row("SELECT engine_id,run_id FROM turn_execution_owners WHERE turn_id=?1 AND executor='graph'",[turn],|r|Ok((r.get(0)?,r.get(1)?))).optional()?;
        let mut owners: Vec<_> = owner.into_iter().collect();
        owners.extend(db.prepare("SELECT engine_id,run_id FROM graph_helper_requests h WHERE message_id=?1 AND NOT EXISTS(SELECT 1 FROM graph_helper_requests newer WHERE newer.message_id=h.message_id AND newer.operation=h.operation AND newer.rowid>h.rowid) ORDER BY h.rowid")?.query_map([&expected.id],|r|Ok((r.get::<_,String>(0)?,r.get::<_,String>(1)?)))?.collect::<rusqlite::Result<Vec<_>>>()?);
        for (engine_id, run) in owners {
            let Some(engine) = self.engines.get(&engine_id) else {
                continue;
            };
            if let Some(cached) = statuses_by_run.get(&(engine_id.clone(), run.clone())) {
                statuses.extend(
                    cached
                        .iter()
                        .filter(|(source, _)| source == expected)
                        .map(|(_, status)| status.clone()),
                );
                continue;
            }
            let view = engine.inspect(&run).map_err(error)?;
            let tx = db.unchecked_transaction()?;
            let mut reader = graph_store::BorrowedReadStore::new(&tx, self.partition(&engine_id)?)
                .map_err(error)?;
            let nodes: Vec<_> = view
                .artifact
                .definition
                .nodes
                .iter()
                .filter(|(_, definition)| {
                    [
                        translation_graph::operation_contract(),
                        gloss_graph::operation_contract(),
                        feedback_graph::operation_contract(),
                        assessment_graph::operation_contract(),
                        support_graph::Task::Brief.operation(),
                        support_graph::Task::Assistance.operation(),
                        support_graph::explanation::operation_contract(),
                    ]
                    .contains(&definition.operation)
                })
                .collect();
            if nodes.is_empty() {
                continue;
            }
            let inputs: Vec<_> = nodes
                .iter()
                .map(|(node, _)| (node.as_str(), "source"))
                .collect();
            // A page holds an immutable runtime view. Resolve and project each run
            // once, including the status/dependency work shared by both messages.
            let sources = engine
                .read_node_inputs(&run, &inputs, &mut reader)
                .map_err(error)?;
            let mut run_statuses = Vec::new();
            for ((node, definition), source) in nodes.into_iter().zip(sources.iter()) {
                let operation = &definition.operation;
                let Some(source) = source else {
                    continue;
                };
                let source: SourceText = serde_json::from_value(source.clone())?;
                let dependencies = ancestors(&view, node)?;
                let status = if view.nodes[node] == Disposition::Waiting
                    && dependencies
                        .iter()
                        .any(|n| view.nodes[n] == Disposition::Unrequested)
                {
                    None
                } else if view.nodes[node] == Disposition::Blocked
                    && dependencies
                        .iter()
                        .any(|n| view.nodes[n] == Disposition::Unknown)
                {
                    Some("unknown".into())
                } else {
                    state(&view.nodes[node])
                };
                let mut failure = None;
                if matches!(
                    view.nodes[node],
                    Disposition::Failed | Disposition::Unknown | Disposition::Blocked
                ) {
                    let mut responses = Vec::new();
                    for dependency in ancestors(&view, node)? {
                        if let Some(attempt) = view.attempts.get(&dependency).and_then(|a| a.last())
                            && let Some(evidence) = engine
                                .read_execution_evidence(attempt.execution, &mut reader)
                                .map_err(error)?
                        {
                            responses.push(serde_json::json!({"node":dependency,"execution":attempt.execution,"observations":evidence.observations,"capture_failed":evidence.evidence_failure.is_some(),"complete":evidence.complete}));
                        }
                    }
                    failure=Some(serde_json::json!({"message":"Requested assistance did not complete.","diagnostics":{"engine":engine_id,"run":run,"node":node,"state":view.nodes[node],"responses":responses}}).to_string());
                }
                run_statuses.push((
                    source,
                    SourceStatus {
                        operation: operation.clone(),
                        state: status,
                        error: failure,
                        attempt: view
                            .attempts
                            .get(node)
                            .and_then(|items| items.last())
                            .map(|item| serde_json::to_string(&item.id))
                            .transpose()?,
                        node_id: format!(
                            "graph:{}",
                            serde_json::to_string(&(&engine_id, &run, node))?
                        ),
                    },
                ));
            }
            statuses.extend(
                run_statuses
                    .iter()
                    .filter(|(source, _)| source == expected)
                    .map(|(_, status)| status.clone()),
            );
            statuses_by_run.insert((engine_id, run), run_statuses);
        }
        Ok(statuses)
    }

    pub fn message_statuses(&self, db: &Connection, messages: &mut [ChatMessage]) -> Result<()> {
        let mut inputs = SourceStatuses::new();
        for message in messages {
            self.message_status(db, message, &mut inputs)?;
        }
        Ok(())
    }

    fn message_status(
        &self,
        db: &Connection,
        message: &mut ChatMessage,
        inputs: &mut SourceStatuses,
    ) -> Result<()> {
        let expected = SourceText {
            id: message.id.clone(),
            text: message.text.clone(),
        };
        for value in self.source_status_with_cache(db, &message.turn_id, &expected, inputs)? {
            if value.operation == translation_graph::operation_contract() {
                message.translation_state = value.state;
            } else if value.operation == gloss_graph::operation_contract() {
                message.gloss_state = value.state;
                message.gloss_operation_id = Some(value.node_id);
                message.gloss_error = value.error;
            } else if value.operation == feedback_graph::operation_contract() {
                message.feedback_state = value.state;
                message.feedback_error = value.error;
            } else if value.operation == assessment_graph::operation_contract() {
                message.assessment_state = value.state;
                message.assessment_error = value.error;
            } else if value.operation == support_graph::Task::Brief.operation() {
                message.brief_state = value.state;
                message.brief_error = value.error;
            } else if value.operation == support_graph::Task::Assistance.operation() {
                message.suggestions_state = value.state;
                message.suggestions_error = value.error;
            } else {
                message.explanations_state = value.state;
                message.explanations_error = value.error;
            }
        }
        if message.role == "user" && message.feedback_error.is_none() {
            message.feedback_error = message.assessment_error.clone();
        }
        Ok(())
    }
}
