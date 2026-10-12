use super::*;

impl Handlers<'_> {
    fn native_help(
        &mut self,
        message: &str,
        operation: crate::ai::graph::Contract,
        retry: bool,
    ) -> Result<Option<String>> {
        let Some(request) = self
            .graph_runtime
            .resolve_help(self.tx, message, &operation, retry)?
        else {
            return Ok(None);
        };
        self.conversation_scope = Some(request.conversation.clone());
        let operation = request.operation.clone();
        self.graph_help = Some(request);
        Ok(Some(operation))
    }

    pub(super) fn request_message_help(
        &mut self,
        message_id: String,
        help: crate::conversations::execution::MessageHelp,
        retry: bool,
    ) -> Result<String> {
        use crate::conversations::execution::MessageHelp;
        use crate::learning::coaching::{assessment_graph, feedback_graph, support_graph};
        let contract = match help {
            MessageHelp::Assessment => assessment_graph::operation_contract(),
            MessageHelp::Coaching => feedback_graph::operation_contract(),
            MessageHelp::ReplyBrief => support_graph::Task::Brief.operation(),
            MessageHelp::Translation => crate::language::translation_graph::operation_contract(),
            MessageHelp::WordGloss => crate::language::gloss_graph::operation_contract(),
        };
        if let Some(operation) = self.native_help(&message_id, contract, retry)? {
            return Ok(operation);
        }
        Err(missing_owner())
    }

    pub(super) fn reassess_feedback(&mut self, turn_id: String, note: String) -> Result<String> {
        if let Some(request) = self
            .graph_runtime
            .resolve_feedback_context(self.tx, &turn_id, &note)?
        {
            self.conversation_scope = Some(request.conversation.clone());
            self.graph_help = Some(request);
            return Ok(turn_id);
        }

        Err(missing_owner())
    }

    pub(super) fn request_message_speech(
        &mut self,
        message_id: String,
        regenerate: bool,
    ) -> Result<String> {
        if let Some((operation, command)) =
            self.graph_runtime
                .prepare_speech_request(self.tx, &message_id, regenerate)?
        {
            self.graph_speech = command;
            self.conversation_scope = Some(self.tx.query_row(
                "SELECT conversation_id FROM messages WHERE id=?1",
                [&message_id],
                |r| r.get(0),
            )?);
            return Ok(operation);
        }
        Err(missing_owner())
    }

    pub(super) fn cancel_message_speech(&mut self, operation_id: String) -> Result<String> {
        if operation_id.starts_with("graph:") || operation_id.starts_with("graph-request:") {
            let (conversation, command) = self
                .graph_runtime
                .prepare_speech_cancel(self.tx, &operation_id)?;
            self.graph_speech = Some(command);
            self.conversation_scope = Some(conversation);
            return Ok(operation_id);
        }
        Err(missing_owner())
    }

    pub(super) fn request_suggestions(&mut self, message_id: String) -> Result<String> {
        if let Some(operation) = self.native_help(
            &message_id,
            crate::learning::coaching::support_graph::Task::Assistance.operation(),
            false,
        )? {
            return Ok(operation);
        }
        Err(missing_owner())
    }

    pub(super) fn request_explanations(&mut self, message_id: String) -> Result<String> {
        if let Some(operation) = self.native_help(
            &message_id,
            crate::learning::coaching::support_graph::explanation::operation_contract(),
            false,
        )? {
            return Ok(operation);
        }
        Err(missing_owner())
    }
    pub(super) fn retry_reply_help(
        &mut self,
        message_id: String,
        kind: crate::learning::coaching::conversation_support::ReplyHelpKind,
    ) -> Result<String> {
        use crate::learning::coaching::{conversation_support::ReplyHelpKind, support_graph};
        let contract = match kind {
            ReplyHelpKind::Brief => support_graph::Task::Brief.operation(),
            ReplyHelpKind::Grammar => support_graph::explanation::operation_contract(),
            ReplyHelpKind::Replies => support_graph::Task::Assistance.operation(),
        };
        if let Some(operation) = self.native_help(&message_id, contract, true)? {
            return Ok(operation);
        }
        Err(missing_owner())
    }
    pub(super) fn retry_gloss(&mut self, operation_id: String) -> Result<String> {
        if let Some(request) = self
            .graph_runtime
            .resolve_gloss_retry(self.tx, &operation_id)?
        {
            self.conversation_scope = Some(request.conversation.clone());
            let operation = request.operation.clone();
            self.graph_help = Some(request);
            return Ok(operation);
        }
        Err(missing_owner())
    }

    pub(super) fn control_turn(&mut self, turn_id: String, control: TurnControl) -> Result<String> {
        if self.tx.query_row("SELECT EXISTS(SELECT 1 FROM turn_execution_owners WHERE turn_id=?1 AND executor='graph')",[&turn_id],|r|r.get::<_,bool>(0))? {
            self.conversation_scope=Some(self.tx.query_row("SELECT conversation_id FROM turns WHERE id=?1",[&turn_id],|r|r.get(0))?);
            self.graph_control=Some((turn_id.clone(),control));
            return Ok(turn_id);
        }
        Err(missing_owner())
    }

    pub(super) fn set_paused(&mut self, paused: bool) -> Result<String> {
        self.tx
            .execute("UPDATE ai_config SET paused=?1 WHERE singleton=1", [paused])?;
        Ok("execution".into())
    }
}

fn missing_owner() -> AppError {
    AppError::new(ErrorCode::NotFound, "Execution owner is unavailable.")
}
