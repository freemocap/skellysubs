//! Read a whole linear fix history independently of conversation viewport pagination.
use crate::{
    learning::coaching::{
        CoachDecision, CoachObservationView, conversation_support::ConversationFeedback,
    },
    model::*,
    storage::store::Store,
};
use rusqlite::{OptionalExtension, params};
use serde::{Deserialize, Serialize};
use ts_rs::TS;

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct MessageHistory {
    pub conversation_id: String,
    pub root_message_id: String,
    pub current_message_id: String,
    pub versions: Vec<MessageVersionView>,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct MessageVersionView {
    pub message_id: String,
    pub turn_id: String,
    pub previous_message_id: Option<String>,
    pub text: String,
    pub created_at: String,
    pub assessments: Vec<VersionAssessmentState>,
    pub feedback: Option<CoachObservationView>,
    pub coach_decision: Option<CoachDecision>,
    pub conversation_feedback: Option<ConversationFeedback>,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct VersionAssessmentState {
    pub kind: String,
    pub state: String,
    pub attempt_id: Option<String>,
    pub error: Option<String>,
}

impl Store {
    pub fn message_history(&self, conversation: &str, message: &str) -> Result<MessageHistory> {
        let db = &self.connection;
        let turn: String = db.query_row(
            "SELECT m.turn_id FROM messages m WHERE m.id=?1 AND m.conversation_id=?2 AND m.role='user' AND EXISTS(SELECT 1 FROM turn_execution_owners WHERE turn_id=m.turn_id AND channel='persona_reply')",
            params![message, conversation], |r| r.get(0),
        ).optional()?.ok_or_else(|| AppError::new(ErrorCode::NotFound, "Message history is unavailable in this conversation."))?;
        let root: String = db.query_row(
            "WITH RECURSIVE parents(id,previous) AS (SELECT id,replaces_turn_id FROM turns WHERE id=?1 UNION ALL SELECT t.id,t.replaces_turn_id FROM turns t JOIN parents p ON t.id=p.previous) SELECT id FROM parents WHERE previous IS NULL",
            [&turn], |r| r.get(0),
        )?;
        let mut versions = db.prepare(
            "WITH RECURSIVE chain(id,depth) AS (SELECT ?1,0 UNION ALL SELECT t.id,c.depth+1 FROM turns t JOIN chain c ON t.replaces_turn_id=c.id) SELECT m.id,m.turn_id,old.id,m.text,m.created_at FROM chain c JOIN turns t ON t.id=c.id JOIN messages m ON m.turn_id=t.id AND m.role='user' LEFT JOIN messages old ON old.turn_id=t.replaces_turn_id AND old.role='user' ORDER BY c.depth",
        )?.query_map([root], |r| Ok(MessageVersionView {
            message_id: r.get(0)?, turn_id: r.get(1)?, previous_message_id: r.get(2)?, text: r.get(3)?, created_at: r.get(4)?,
            assessments: vec![], feedback: None, coach_decision: None, conversation_feedback: None,
        }))?.collect::<rusqlite::Result<Vec<_>>>()?;
        for version in &mut versions {
            let context = super::assessments::context(db, &version.turn_id)?;
            version.feedback = crate::learning::coaching::coach_policy::view(&context)?;
            version.coach_decision = context
                .get("coachDecision")
                .cloned()
                .map(serde_json::from_value)
                .transpose()?;
            version.conversation_feedback = super::assessments::feedback(db, &version.turn_id)?;
            version.assessments = db.prepare(
                "SELECT a.kind,a.attempt_id FROM conversation_graph_assessments a WHERE a.turn_id=?1 AND NOT EXISTS(SELECT 1 FROM conversation_graph_assessments newer WHERE newer.turn_id=a.turn_id AND newer.kind=a.kind AND newer.rowid>a.rowid) ORDER BY a.kind",
            )?.query_map([&version.turn_id], |r| Ok(VersionAssessmentState {kind:r.get(0)?,state:"succeeded".into(),attempt_id:r.get(1)?,error:None}))?
                .collect::<rusqlite::Result<Vec<_>>>()?;
            let source = crate::language::source_graph::SourceText {
                id: version.message_id.clone(),
                text: version.text.clone(),
            };
            for status in self
                .graph_runtime
                .source_status(db, &version.turn_id, &source)?
            {
                use crate::learning::coaching::{assessment_graph, feedback_graph};
                let kind = if status.operation == assessment_graph::operation_contract() {
                    "skill_assessment"
                } else if status.operation == feedback_graph::operation_contract() {
                    "coach_feedback"
                } else {
                    continue;
                };
                version.assessments.retain(|item| item.kind != kind);
                if let Some(state) = status.state {
                    version.assessments.push(VersionAssessmentState {
                        kind: kind.into(),
                        state,
                        attempt_id: status.attempt,
                        error: status.error,
                    });
                }
            }
            version.assessments.sort_by(|a, b| a.kind.cmp(&b.kind));
        }
        let root_message_id = versions
            .first()
            .ok_or_else(|| {
                AppError::new(ErrorCode::Storage, "Message history has no source version.")
            })?
            .message_id
            .clone();
        let current_message_id = versions
            .last()
            .expect("nonempty history")
            .message_id
            .clone();
        Ok(MessageHistory {
            conversation_id: conversation.into(),
            root_message_id,
            current_message_id,
            versions,
        })
    }
}
