//! Current domain authority for declared conversation reply effects. The native reducer owns
//! readiness, cancellation, producer/consumer identity and adoption eligibility.
//! Call under workspace serialization and retain returned AppError diagnostics.
use super::{prose, source_authority};
use crate::ai::{
    connections::{access, configuration},
    graph::{Authority, CommitIntent, CommitRequest, Resource},
};
use crate::model::{AppError, ErrorCode, Result};
use rusqlite::{Connection, OptionalExtension, params};
mod helpers;
pub use helpers::check_helper;

#[derive(Clone, Copy)]
pub enum Phase {
    Dispatch,
    SteppedDispatch,
    Adoption,
}

fn rejected(reason: &str) -> AppError {
    AppError::new(
        ErrorCode::Conflict,
        "The graph request no longer has conversation authority.",
    )
    .with_diagnostics(serde_json::json!({"stage":"graph_conversation_authority","reason":reason}))
}

/// Validate the actual provider Work before staging transport identity. A failed
/// check leaves the invocation unexposed when used by the native commit adapter.
pub fn dispatch(db: &Connection, request: &CommitRequest<'_>) -> Result<()> {
    dispatch_with_step(db, request, false)
}

pub fn dispatch_with_step(
    db: &Connection,
    request: &CommitRequest<'_>,
    stepping: bool,
) -> Result<()> {
    let CommitIntent::Dispatch {
        authority, work, ..
    } = &request.intent
    else {
        return Err(rejected("dispatch_required"));
    };
    if work.resource != Resource::Provider || work.artifact != authority.artifact {
        return Err(rejected("provider_work_mismatch"));
    }
    if work.operation != prose::operation_contract() {
        return check_helper(
            db,
            &request.next.stamp().engine,
            authority,
            work,
            if stepping {
                Phase::SteppedDispatch
            } else {
                Phase::Dispatch
            },
        );
    }
    let captured = prose::decode(&work.inputs).map_err(|fault| {
        rejected("captured_input_invalid").with_diagnostics(serde_json::json!({
            "stage":"graph_conversation_authority", "reason":"captured_input_invalid",
            "code":fault.code, "path":fault.path,
        }))
    })?;
    check_owner(
        db,
        &request.next.stamp().engine,
        authority,
        &captured,
        if stepping {
            Phase::SteppedDispatch
        } else {
            Phase::Dispatch
        },
    )
}

/// Also used after credential/protocol awaits and at adoption, with the original
/// native producer inputs. Do not rebuild them from current settings. The host
/// must separately recheck native cancellation/current-consumer state after awaits.
/// Pausing prevents new dispatch, but does not revoke an already available result.
pub fn check_owner(
    db: &Connection,
    engine: &str,
    authority: &Authority<'_>,
    captured: &prose::Request,
    phase: Phase,
) -> Result<()> {
    if db.is_autocommit() {
        return Err(rejected("transaction_required"));
    }
    if authority.node.is_none() {
        return Err(rejected("reply_owner_required"));
    }
    let owner: Option<(String, bool, bool)> = db.query_row(
        "SELECT t.id,t.paused,t.refusal_hold IS NOT NULL FROM turn_execution_owners o JOIN turns t ON t.id=o.turn_id JOIN conversations c ON c.id=t.conversation_id JOIN contacts contact ON contact.id=c.contact_id JOIN conversation_graph_effects e ON e.turn_id=t.id WHERE o.executor='graph' AND o.channel IN ('coach','persona_reply','persona_opening') AND o.engine_id=?1 AND o.run_id=?2 AND o.artifact_id=?3 AND e.node_key=?4 AND e.scope=?5 AND e.role=CASE o.channel WHEN 'coach' THEN 'coach_reply' ELSE o.channel END AND c.archived=0 AND contact.archived=0 AND t.state IN ('pending','assisting')",
        params![engine, authority.run, authority.artifact, authority.node, authority.scope],
        |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?))).optional()?;
    let (turn, paused, held) = owner.ok_or_else(|| rejected("owner_revoked"))?;
    if !matches!(phase, Phase::Adoption)
        && ((paused && !matches!(phase, Phase::SteppedDispatch))
            || held
            || configuration::config(db)?.paused)
    {
        return Err(rejected("dispatch_paused"));
    }
    source_authority::check(db, &turn, &captured.context.source_ids)?;
    access::check_captured(db, access::Capability::Chat, &captured.target)
}
