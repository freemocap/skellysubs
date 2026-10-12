//! Workspace-wide activation policy. Captured by each conversation turn.
use serde::{Deserialize, Serialize};
use ts_rs::TS;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
pub enum ExecutionMode {
    Automatic,
    OnDemand,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ExecutionPreferences {
    pub assessment: ExecutionMode,
    pub reply_brief: ExecutionMode,
    pub reading: ExecutionMode,
}

impl Default for ExecutionPreferences {
    fn default() -> Self {
        Self {
            assessment: ExecutionMode::Automatic,
            reply_brief: ExecutionMode::OnDemand,
            reading: ExecutionMode::Automatic,
        }
    }
}

impl ExecutionPreferences {
    pub fn automatic(&self, kind: &str) -> bool {
        let mode = match kind {
            "skill_assessment" | "skill_attribution" => self.assessment,
            "reply_brief" => self.reply_brief,
            "persona_word_gloss" | "user_word_gloss" | "reply_translation" | "user_translation" => {
                self.reading
            }
            _ => return true,
        };
        mode == ExecutionMode::Automatic
    }
}
