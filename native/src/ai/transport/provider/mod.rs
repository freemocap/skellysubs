mod payload;
mod request;
mod response;

pub use payload::{
    GLOSS_OUTPUT_TOKENS, MAX_OUTPUT_TOKENS, RequestOutput, dispatch_payload, payload,
    payload_with_output, structured_output, validate_temperature,
};
pub use request::{client, complete, complete_with_output};
pub use response::{decode, strip_prose_emojis, validate_prose};

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PromptMessage {
    pub role: String,
    pub content: String,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Completion {
    pub diagnostics: Option<serde_json::Value>,
    pub text: String,
    pub finish_reason: String,
    pub actual_model: String,
    pub provider_id: String,
    pub input_tokens: Option<i32>,
    pub output_tokens: Option<i32>,
}

pub(crate) mod stream;
