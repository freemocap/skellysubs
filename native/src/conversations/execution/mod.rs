//! Durable conversation execution: admission, dispatch and source-bound publication.
use crate::ai::transport::provider::PromptMessage;
use crate::model::*;
use crate::storage::store::Store;
use rusqlite::{Connection, OptionalExtension, params};
use uuid::Uuid;

mod admission;
pub mod coach_graph;
mod connections;
pub mod context;
pub mod graph_authority;
pub mod graph_publication;
pub(crate) mod graph_runtime;
pub mod graph_text_request;
mod holds;
mod optional_help;
pub use optional_help::MessageHelp;
pub mod partner_graph;
pub mod prose;
pub mod reply_reading_graph;
mod snapshots;
mod source_authority;
mod speech;
mod turns;

use crate::ai::connections::configuration::active_credential;
use crate::ai::connections::configuration::config;
use admission::TURN_ATTEMPT_LIMIT;
pub(crate) use admission::admit_network_work;
use admission::budget_error;
pub(crate) use connections::invalidate;
use holds::pause_related;
use holds::release_hold;
pub use turns::accept_coach;
pub(crate) use turns::accept_opening;
pub(crate) use turns::accept_revision_send;
pub(crate) use turns::capture_native_send;

use crate::ai::audio::speech_input;

fn fail(message: &str) -> AppError {
    AppError::new(ErrorCode::Validation, message)
}

fn id() -> String {
    Uuid::new_v4().to_string()
}

fn bump(db: &Connection) -> Result<()> {
    db.execute("UPDATE metadata SET revision=revision+1", [])?;
    Ok(())
}

#[cfg(test)]
mod tests;
