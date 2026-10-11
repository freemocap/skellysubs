//! Reassessment captures a learner note in a fresh invocation of shared feedback.
use super::*;
use crate::learning::coaching::feedback_graph;
fn unavailable() -> AppError {
    AppError::new(
        ErrorCode::Conflict,
        "Message feedback is unavailable or still running.",
    )
}
impl Runtime {
    pub fn resolve_feedback_context(
        &self,
        db: &Connection,
        turn: &str,
        note: &str,
    ) -> Result<Option<HelpRequest>> {
        let note = note.trim();
        if note.is_empty() || note.chars().count() > 2000 || note.contains('\0') {
            return Err(AppError::new(
                ErrorCode::Validation,
                "Context must contain 1–2000 characters.",
            ));
        }
        let native:bool=db.query_row("SELECT EXISTS(SELECT 1 FROM turn_execution_owners WHERE turn_id=?1 AND executor='graph')",[turn],|r|r.get(0))?;
        if !native {
            return Ok(None);
        }
        let message: String = db.query_row(
            "SELECT id FROM messages WHERE turn_id=?1 AND role='user'",
            [turn],
            |r| r.get(0),
        )?;
        let contract = feedback_graph::operation_contract();
        let mut request = self
            .resolve_help(db, &message, &contract, false)?
            .ok_or_else(unavailable)?;
        let engine = self.engines.get(&request.engine).ok_or_else(unavailable)?;
        let view = engine.inspect(&request.run).map_err(error)?;
        if !matches!(
            view.nodes[&request.node],
            Disposition::Adopted | Disposition::Failed | Disposition::Unknown
        ) {
            return Err(unavailable());
        }
        let mut reader = graph_store::BorrowedReadStore::new(db, self.partition(&request.engine)?)
            .map_err(error)?;
        let mut inputs = Values::new();
        for port in feedback_graph::inputs().keys() {
            inputs.insert(
                port.clone(),
                engine
                    .read_node_input(&request.run, &request.node, port, &mut reader)
                    .map_err(error)?
                    .ok_or_else(unavailable)?,
            );
        }
        let context = inputs
            .get_mut("context")
            .and_then(serde_json::Value::as_object_mut)
            .ok_or_else(unavailable)?;
        context.insert("feedbackContext".into(), serde_json::json!(note));
        let captured = feedback_graph::decode(&inputs).map_err(error)?;
        if captured.source != request.source {
            return Err(unavailable());
        }
        super::super::admit_network_work(db, self, 1)?;
        request.fresh = Some(super::helper_requests::Fresh {
            run: uuid::Uuid::new_v4().to_string(),
            turn: turn.into(),
            conversation: request.conversation.clone(),
            source: request.source.clone(),
            inputs,
            operation: contract,
        });
        Ok(Some(request))
    }
}
