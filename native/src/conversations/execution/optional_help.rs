//! Optional work is source-bound, deduplicated and only retried explicitly.
use serde::{Deserialize, Serialize};
use ts_rs::TS;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
pub enum MessageHelp {
    Assessment,
    Coaching,
    ReplyBrief,
    Translation,
    WordGloss,
}
