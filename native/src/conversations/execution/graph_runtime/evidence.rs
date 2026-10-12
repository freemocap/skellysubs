//! Fresh local evidence snapshots use current native adoption facts and exact
//! source ownership. Missing evidence stays absent; no assessment is requested.
use super::*;
use crate::{
    language::source_graph::SourceText,
    learning::coaching::{
        attribution_graph,
        support_graph::{explanation, explanation_snapshot},
    },
};
use rusqlite::OptionalExtension;

impl Runtime {
    pub fn available_evidence(
        &self,
        db: &Connection,
        identity: &InvocationIdentity,
        source: &SourceText,
    ) -> Result<Option<explanation::Evidence>> {
        let rejected = || {
            AppError::new(
                ErrorCode::Conflict,
                "Evidence snapshot source is no longer authorized.",
            )
        };
        let engine_id = identity.engine.as_deref().ok_or_else(rejected)?;
        if identity.operation != explanation_snapshot::operation_contract() {
            return Err(rejected());
        }
        let engine = self.engines.get(engine_id).ok_or_else(rejected)?;
        let tx = db.unchecked_transaction()?;
        let work = {
            let mut reader = graph_store::BorrowedReadStore::new(&tx, self.partition(engine_id)?)
                .map_err(error)?;
            engine
                .read_execution_work(identity.execution, &mut reader)
                .map_err(error)?
        };
        if work.operation != identity.operation
            || work.artifact != identity.artifact
            || work.inputs.get("source") != Some(&serde_json::to_value(source)?)
        {
            return Err(rejected());
        }
        let owner:Option<(String,String,String)> = tx.query_row(
            "SELECT t.id,o.run_id,t.context FROM turn_execution_owners o JOIN turns t ON t.id=o.turn_id JOIN messages m ON m.turn_id=t.id JOIN conversations c ON c.id=t.conversation_id JOIN contacts contact ON contact.id=c.contact_id WHERE o.executor='graph' AND o.engine_id=?1 AND o.artifact_id=?2 AND m.id=?3 AND m.text=?4 AND m.role='user' AND c.archived=0 AND contact.archived=0 AND t.state NOT IN ('cancelled','invalidated') AND NOT EXISTS(SELECT 1 FROM turns child WHERE child.replaces_turn_id=t.id)",
            params![engine_id,identity.artifact,source.id,source.text],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?))).optional()?;
        let (turn, run, raw) = owner.ok_or_else(rejected)?;
        let view = engine.inspect(&run).map_err(error)?;
        if !view.attempts.values().any(|attempts| {
            attempts.last().is_some_and(|a| {
                a.execution == identity.execution && matches!(a.state, AttemptState::Running)
            })
        }) {
            return Err(rejected());
        }
        let Some((_, assessment)) =
            crate::conversations::assessments::current(&tx, &turn, "skill_assessment")?
        else {
            return Ok(None);
        };
        let context: serde_json::Value = serde_json::from_str(&raw)?;
        let mut spans = BTreeMap::new();
        for (node, definition) in &view.artifact.definition.nodes {
            if definition.operation != attribution_graph::operation_contract()
                || view.nodes[node] != Disposition::Adopted
            {
                continue;
            }
            let attempt = view.attempts[node].last().ok_or_else(rejected)?;
            let expected = format!("graph:{}", serde_json::to_string(&(engine_id, attempt.id))?);
            if context["skillAttributionAttempt"] != expected {
                continue;
            }
            let skills = context["skillAttribution"]["skills"]
                .as_object()
                .ok_or_else(rejected)?;
            for (id, value) in skills {
                spans.insert(id.clone(), serde_json::from_value(value["spans"].clone())?);
            }
        }
        Ok(Some(explanation::Evidence {
            source: source.clone(),
            presence: serde_json::from_value(assessment["presence"].clone())?,
            spans,
        }))
    }
}
