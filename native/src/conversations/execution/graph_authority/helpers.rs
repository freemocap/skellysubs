//! Authority for the actual source-bound operation, independent of node labels.
use super::*;
use crate::{
    ai::{connections::access::ResolvedTarget, graph::Work},
    language::{gloss_graph, source_graph::SourceText, translation_graph},
    learning::coaching::{assessment_graph, attribution_graph, feedback_graph, support_graph},
};

fn captured(work: &Work) -> Result<(ResolvedTarget, Vec<SourceText>)> {
    let input = &work.inputs;
    let decode_error = |fault: crate::ai::graph::Fault| {
        rejected("captured_input_invalid").with_diagnostics(serde_json::json!({
        "stage":"graph_conversation_authority", "reason":"captured_input_invalid", "code":fault.code, "path":fault.path,
    }))
    };
    let pair = if work.operation == crate::speech::synthesis_graph::operation_contract()
        || work.operation == crate::speech::synthesis_graph::playback::lookup_operation()
    {
        let request = crate::speech::synthesis_graph::decode(input).map_err(decode_error)?;
        (request.settings.target, vec![request.source])
    } else if work.operation == translation_graph::operation_contract() {
        let request = translation_graph::decode(input).map_err(decode_error)?;
        (request.target, vec![request.source])
    } else if work.operation == gloss_graph::operation_contract() {
        let request = gloss_graph::decode(input).map_err(decode_error)?;
        (request.target, vec![request.source])
    } else if work.operation == assessment_graph::operation_contract() {
        // Authority checks source and access. The operation validates optional
        // captured criteria before its provider callback; invalid criteria must
        // fail this branch, not reject dispatch for the entire host scheduler.
        let transport = crate::ai::transport::graph_text::decode(input).map_err(decode_error)?;
        let source: SourceText = serde_json::from_value(
            input
                .get("source")
                .ok_or_else(|| rejected("captured_source_missing"))?
                .clone(),
        )?;
        (transport.target, vec![source])
    } else if work.operation == feedback_graph::operation_contract() {
        let request = feedback_graph::decode(input).map_err(decode_error)?;
        (request.target, vec![request.source])
    } else if work.operation == attribution_graph::operation_contract() {
        let request = attribution_graph::decode(input).map_err(decode_error)?;
        (request.target, vec![request.selection.source])
    } else if work.operation == support_graph::explanation::operation_contract() {
        let request = support_graph::explanation::decode(input).map_err(decode_error)?;
        let mut sources = vec![request.exchange.source];
        sources.extend(request.exchange.learner);
        sources.extend(request.learning.evidence.map(|e| e.source));
        (request.exchange.target, sources)
    } else if let Some(task) = [support_graph::Task::Brief, support_graph::Task::Assistance]
        .into_iter()
        .find(|task| task.operation() == work.operation)
    {
        let request = support_graph::decode(task, input).map_err(decode_error)?;
        let mut sources = vec![request.source];
        sources.extend(request.learner);
        (request.target, sources)
    } else {
        return Err(rejected("unsupported_helper_operation"));
    };
    Ok(pair)
}

pub fn check_helper(
    db: &Connection,
    engine: &str,
    authority: &Authority<'_>,
    work: &Work,
    phase: Phase,
) -> Result<()> {
    if db.is_autocommit() || authority.node.is_none() || work.artifact != authority.artifact {
        return Err(rejected("helper_transaction_owner_required"));
    }
    let (target, sources) = captured(work)?;
    type Owner = (String, bool, bool, String);
    let owner: Option<Owner> = db.query_row(
        "SELECT t.id,CASE WHEN o.channel IN ('speech','helper') THEN 0 ELSE t.paused END,t.refusal_hold IS NOT NULL,t.context FROM graph_conversation_runs o JOIN turns t ON t.id=o.turn_id JOIN conversations c ON c.id=t.conversation_id JOIN contacts contact ON contact.id=c.contact_id WHERE o.channel IN ('persona_reply','persona_opening','speech','helper') AND o.engine_id=?1 AND o.run_id=?2 AND o.artifact_id=?3 AND o.scope=?4 AND c.archived=0 AND contact.archived=0 AND t.state NOT IN ('cancelled','invalidated') AND NOT EXISTS(SELECT 1 FROM turns child WHERE child.replaces_turn_id=t.id)",
        params![engine,authority.run,authority.artifact,authority.scope], |r|Ok((r.get(0)?,r.get(1)?,r.get(2)?,r.get(3)?))).optional()?;
    let (turn, paused, held, raw) = owner.ok_or_else(|| rejected("owner_revoked"))?;
    let superseded: bool = db.query_row("SELECT EXISTS(SELECT 1 FROM graph_helper_requests h WHERE h.engine_id=?1 AND h.run_id=?2 AND (h.operation!=?3 OR EXISTS(SELECT 1 FROM graph_helper_requests newer WHERE newer.message_id=h.message_id AND newer.operation=h.operation AND newer.rowid>h.rowid)))",params![engine,authority.run,serde_json::to_string(&work.operation)?],|r|r.get(0))?;
    if superseded {
        return Err(rejected("helper_superseded"));
    }

    if !matches!(phase, Phase::Adoption)
        && ((paused && !matches!(phase, Phase::SteppedDispatch))
            || held
            || configuration::config(db)?.paused)
    {
        return Err(rejected("dispatch_paused"));
    }
    for source in sources {
        let current: bool = db.query_row(
            "SELECT EXISTS(SELECT 1 FROM messages WHERE id=?1 AND text=?2 AND turn_id=?3)",
            params![source.id, source.text, turn],
            |r| r.get(0),
        )?;
        if !current {
            return Err(rejected("source_replaced"));
        }
    }
    let captured: serde_json::Value = serde_json::from_str(&raw)?;
    let auxiliary: bool = db.query_row(
        "SELECT EXISTS(SELECT 1 FROM graph_speech_requests WHERE engine_id=?1 AND run_id=?2 UNION ALL SELECT 1 FROM graph_helper_requests WHERE engine_id=?1 AND run_id=?2)",
        params![engine, authority.run],
        |r| r.get(0),
    )?;
    let history: Vec<String> = serde_json::from_value(captured["sourceIds"].clone())?;
    if !auxiliary {
        source_authority::check(db, &turn, &history)?;
    }
    let capability = if work.operation == crate::speech::synthesis_graph::operation_contract()
        || work.operation == crate::speech::synthesis_graph::playback::lookup_operation()
    {
        access::Capability::Speech
    } else {
        access::Capability::Chat
    };
    access::check_captured(db, capability, &target)
}
