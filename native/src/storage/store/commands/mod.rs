use super::*;

mod assistance;
mod conversations;
mod guide_actions;
mod learning;
mod native_execution;
mod partners;
mod phrase;
mod skill_start;

struct Handlers<'a> {
    graph_runtime: &'a crate::conversations::execution::graph_runtime::Runtime,
    graph_speech: Option<crate::conversations::execution::graph_runtime::SpeechCommand>,
    graph_help: Option<crate::conversations::execution::graph_runtime::HelpRequest>,
    graph_admission: Option<(
        String,
        crate::conversations::execution::graph_runtime::Admission,
    )>,
    graph_control: Option<(String, TurnControl)>,
    tx: &'a Connection,
    config: &'a crate::configuration::Registry,
    snapshot: &'a Snapshot,
    persona_scope: Option<String>,
    conversation_scope: Option<String>,
}

impl Store {
    pub fn execute(&mut self, command: Command) -> Result<Receipt> {
        if command.session_id != self.session_id {
            return Err(AppError::new(
                ErrorCode::SessionExpired,
                "The application session changed. Refresh before continuing.",
            ));
        }
        Uuid::parse_str(&command.action_id)
            .map_err(|_| AppError::new(ErrorCode::Validation, "Invalid action identity."))?;
        let request = serde_json::to_string(&command.action)?;
        let tx = self.connection.transaction()?;
        let previous: Option<(String, String)> = tx
            .query_row(
                "SELECT request,receipt FROM receipts WHERE action_id=?1",
                [&command.action_id],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .optional()?;
        if let Some((stored_request, receipt)) = previous {
            if stored_request != request {
                return Err(AppError::new(
                    ErrorCode::Conflict,
                    "An action ID cannot be reused for another action.",
                ));
            }
            return Ok(serde_json::from_str(&receipt)?);
        }
        native_execution::check(&tx, &command.action)?;
        let snapshot = read_snapshot(&tx, &self.session_id, &self.config)?;
        let revision = snapshot
            .revision
            .checked_add(1)
            .ok_or_else(|| AppError::new(ErrorCode::Storage, "Revision capacity exceeded."))?;
        let mut handlers = Handlers {
            graph_runtime: &self.graph_runtime,
            graph_help: None,
            graph_speech: None,
            graph_admission: None,
            graph_control: None,
            tx: &tx,
            config: &self.config,
            snapshot: &snapshot,
            persona_scope: None,
            conversation_scope: None,
        };
        let entity_id = match command.action {
            Action::AskGuideCoach {
                conversation_id,
                text,
                guide,
                focus,
                expected_revision,
            } => {
                handlers.ask_guide_coach(conversation_id, text, guide, focus, expected_revision)?
            }
            Action::StartGuideConversation {
                source_conversation_id,
                guide,
                example,
                phrase,
                expected_revision,
            } => handlers.start_guide_conversation(
                source_conversation_id,
                guide,
                example,
                phrase,
                expected_revision,
            )?,
            Action::StartSkillConversation {
                source_conversation_id,
                language,
                variety,
                skill_id,
                subskill_id,
                expected_revision,
            } => handlers.start_skill_conversation(
                source_conversation_id,
                language,
                variety,
                crate::conversations::direction::TopicChoice::Skill {
                    skill_id,
                    subskill_id,
                },
                expected_revision,
            )?,
            Action::StartPhraseConversation {
                source_message_id,
                phrase,
                contact_id,
                expected_revision,
            } => handlers.start_phrase_conversation(
                source_message_id,
                phrase,
                contact_id,
                expected_revision,
            )?,
            Action::RequestMessageHelp {
                message_id,
                help,
                retry,
            } => handlers.request_message_help(message_id, help, retry)?,
            Action::StartConversation {
                conversation_id,
                configuration,
                message,
                input,
                expected_revision,
            } => handlers.start_conversation(
                conversation_id,
                configuration,
                message,
                input,
                expected_revision,
            )?,
            Action::UpdateConversationPrompt {
                conversation_id,
                configuration,
                additions,
                deletions,
                expected_revision,
                expected_settings_revision,
            } => {
                crate::conversations::saved_topics::update(
                    handlers.tx,
                    handlers.snapshot,
                    &additions,
                    &deletions,
                    expected_revision,
                )?;
                let current = handlers
                    .snapshot
                    .conversations
                    .iter()
                    .find(|c| c.id == conversation_id)
                    .ok_or_else(missing)?;
                let settings = crate::conversations::direction::settings(
                    handlers.config,
                    &current.language_id,
                    &current.settings,
                    &configuration,
                )?;
                handlers.update_settings(conversation_id, expected_settings_revision, settings)?
            }
            Action::SaveTopics {
                additions,
                deletions,
                expected_revision,
            } => crate::conversations::saved_topics::update(
                handlers.tx,
                handlers.snapshot,
                &additions,
                &deletions,
                expected_revision,
            )?,
            Action::CoachControl { turn_id, control } => {
                handlers.coach_control(turn_id, control)?
            }
            Action::ReviseTurn {
                conversation_id,
                turn_id,
                text,
                input,
                expected_revision,
            } => handlers.revise_turn(conversation_id, turn_id, text, input, expected_revision)?,
            Action::AskCoach {
                conversation_id,
                text,
                expected_revision,
            } => handlers.ask_coach(conversation_id, text, expected_revision)?,
            Action::SendMessage {
                input,
                conversation_id,
                text,
                expected_revision,
            } => handlers.send_message(input, conversation_id, text, expected_revision)?,
            Action::RequestMessageSpeech {
                message_id,
                regenerate,
            } => handlers.request_message_speech(message_id, regenerate.unwrap_or(false))?,
            Action::CancelMessageSpeech { operation_id } => {
                handlers.cancel_message_speech(operation_id)?
            }
            Action::RequestExplanations { message_id } => {
                handlers.request_explanations(message_id)?
            }
            Action::ReassessFeedback { turn_id, note } => {
                handlers.reassess_feedback(turn_id, note)?
            }
            Action::RetryReplyHelp {
                message_id,
                help_kind,
            } => handlers.retry_reply_help(message_id, help_kind)?,
            Action::RequestSuggestions { message_id } => {
                handlers.request_suggestions(message_id)?
            }
            Action::RetryGloss { operation_id } => handlers.retry_gloss(operation_id)?,
            Action::ControlTurn { turn_id, control } => handlers.control_turn(turn_id, control)?,
            Action::SetPaused { paused } => handlers.set_paused(paused)?,
            Action::StartChat { language_id } => handlers.start_chat(language_id)?,
            Action::CreateContact {
                language_id,
                details,
            } => handlers.create_contact(language_id, details)?,
            Action::UpdatePersona {
                persona_id,
                expected_revision,
                details,
            } => handlers.update_persona(persona_id, expected_revision, details)?,
            Action::SetContactArchived {
                contact_id,
                expected_revision,
                archived,
            } => handlers.set_contact_archived(contact_id, expected_revision, archived)?,
            Action::DeleteContact {
                contact_id,
                expected_revision,
            } => handlers.delete_contact(contact_id, expected_revision)?,
            Action::CreateConversation { contact_id, title } => {
                handlers.create_conversation(contact_id, title)?
            }
            Action::OpenConversation { conversation_id } => {
                handlers.open_conversation(conversation_id)?
            }
            Action::UpdateConversation {
                conversation_id,
                expected_revision,
                title,
                archived,
            } => {
                handlers.update_conversation(conversation_id, expected_revision, title, archived)?
            }
            Action::UpdateSettings {
                conversation_id,
                expected_revision,
                settings,
            } => handlers.update_settings(conversation_id, expected_revision, settings)?,
            Action::DeleteConversation {
                conversation_id,
                expected_revision,
            } => handlers.delete_conversation(conversation_id, expected_revision)?,
            Action::UpdateLearner {
                expected_revision,
                name,
                preferences,
            } => handlers.update_learner(expected_revision, name, preferences)?,
        };
        tx.execute(
            "UPDATE metadata SET revision=?1 WHERE singleton=1",
            [revision],
        )?;
        let receipt = Receipt {
            action_id: command.action_id,
            entity_id,
            revision,
        };
        tx.execute(
            "INSERT INTO receipts VALUES(?1,?2,?3,?4,?5)",
            params![
                receipt.action_id,
                request,
                serde_json::to_string(&receipt)?,
                handlers.persona_scope,
                handlers.conversation_scope
            ],
        )?;
        let graph_admission = handlers.graph_admission;
        let graph_control = handlers.graph_control;
        let graph_help = handlers.graph_help;
        let graph_speech = handlers.graph_speech;
        if let Some((turn, admission)) = graph_admission {
            self.graph_runtime.admit(tx, &turn, admission)?;
        } else if let Some((turn, control)) = graph_control {
            self.graph_runtime.control(tx, &turn, control)?;
        } else if let Some(request) = graph_help {
            self.graph_runtime.request_help(tx, request)?;
        } else if let Some(request) = graph_speech {
            self.graph_runtime.apply_speech_command(tx, request)?;
        } else {
            tx.commit()?;
        }
        self.graph_runtime.prune(&self.connection)?;
        Ok(receipt)
    }
}
