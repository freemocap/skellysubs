//! Assessment receipts, disclosure ownership and deterministic learning effects
//! borrow the same transaction as native adoption.
use super::*;
use crate::{
    ai::graph::Work,
    language::source_graph::SourceText,
    learning::{
        coaching::{assessment_graph, feedback_graph},
        learner::progression,
        practice, rewards,
    },
};
use serde_json::{Value, json};
use std::collections::{BTreeMap, BTreeSet};

fn projection(
    operation: &crate::ai::graph::Contract,
) -> Option<(&'static str, &'static str, &'static str)> {
    if *operation == assessment_graph::operation_contract() {
        Some(("skill_assessment", "assessment", "assessment"))
    } else if *operation == feedback_graph::operation_contract() {
        Some(("coach_feedback", "feedback", "value"))
    } else {
        None
    }
}

pub(super) fn surface(operation: &crate::ai::graph::Contract, port: &str) -> Option<&'static str> {
    let (kind, output, _) = projection(operation)?;
    (port == output).then_some(kind)
}

pub fn publish_assessment(
    db: &Connection,
    request: &CommitRequest<'_>,
    work: &Work,
    registry: &crate::configuration::Registry,
    session: &str,
    authorize: impl FnOnce(&Connection, &Authority<'_>) -> Result<()>,
) -> Result<bool> {
    transaction(db)?;
    let Some((kind, port, field)) = projection(&work.operation) else {
        return Ok(false);
    };
    let CommitIntent::Adopt {
        authority,
        attempt,
        execution,
        values,
    } = &request.intent
    else {
        return Err(rejected());
    };
    let bound = values.get(port).ok_or_else(rejected)?;
    let source: SourceText = serde_json::from_value(bound["source"].clone())?;
    let captured: SourceText =
        serde_json::from_value(work.inputs.get("source").ok_or_else(rejected)?.clone())?;
    let owner = source::adopted_owner(db, request, work, &source, &captured)?;
    if owner.role != "user" {
        return Err(rejected());
    }
    authorize(db, authority)?;
    let mut value = bound.get(field).ok_or_else(rejected)?.clone();
    let receipt = format!(
        "graph-assessment:{}",
        serde_json::to_string(&(&request.next.stamp().engine, attempt))?
    );
    let learning = if kind == "skill_assessment" {
        let captured =
            assessment_graph::decode(&work.inputs).map_err(super::super::graph_runtime::error)?;
        value["providerMode"] = serde_json::to_value(captured.target.route)?;
        let expected: BTreeSet<String> = captured
            .content
            .skills
            .iter()
            .map(|s| s.id.clone())
            .collect();
        let presence: BTreeMap<String, practice::Presence> =
            serde_json::from_value(value["presence"].clone())?;
        if presence.keys().cloned().collect::<BTreeSet<_>>() != expected || expected.is_empty() {
            return Err(rejected());
        }
        let (target,sequence):(String,i32) = db.query_row("SELECT c.language_id,m.sequence FROM messages m JOIN conversations c ON c.id=m.conversation_id WHERE m.id=?1",[&source.id],|r|Ok((r.get(0)?,r.get(1)?)))?;
        let before = progression::snapshot_db(db, registry, session, &target)?;
        rewards::skill_level_events::synchronize(db, &before, None)?;
        Some((presence, expected, target, sequence))
    } else {
        feedback_graph::decode(&work.inputs).map_err(super::super::graph_runtime::error)?;
        None
    };
    db.execute("INSERT INTO conversation_graph_assessments(id,turn_id,node_key,attempt_id,execution_id,message_id,kind,result,engine_id,run_id) VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9,?10)",
        params![receipt,owner.turn,owner.node,serde_json::to_string(attempt)?,serde_json::to_string(execution)?,source.id,kind,value.to_string(),request.next.stamp().engine,authority.run])?;
    if kind == "coach_feedback" {
        let captured =
            feedback_graph::decode(&work.inputs).map_err(super::super::graph_runtime::error)?;
        if let Some(note) = captured
            .context
            .feedback_context
            .as_deref()
            .filter(|n| !n.is_empty())
        {
            crate::learning::effort::exploration::graph_feedback(db, &owner.turn, note)?;
        }
    }
    if let Some((presence, expected, target, sequence)) = learning {
        practice::publish_graph(db, &owner.turn, &receipt, presence, &expected)?;
        db.execute("UPDATE turns SET context=json_set(context,'$.skillAssessment',json(?2),'$.skillAssessmentAttempt',?3) WHERE id=?1",params![owner.turn,value.to_string(),receipt])?;
        let answer = &value["understandability"];
        let reaction = match answer["choice"].as_str() {
            Some("understandable") => json!({"kind":"understood","answer":answer}),
            Some("needs_clarification" | "unrecoverable") => {
                json!({"kind":"confused","answer":answer})
            }
            _ => Value::Null,
        };
        db.execute(
            "UPDATE turns SET context=json_set(context,'$.partnerReaction',json(?2)) WHERE id=?1",
            params![owner.turn, reaction.to_string()],
        )?;
        rewards::publish(db, &owner.turn, &receipt)?;
        crate::learning::effort::message(db, &owner.turn, kind)?;
        let after = progression::snapshot_db(db, registry, session, &target)?;
        rewards::skill_level_events::synchronize(
            db,
            &after,
            Some(rewards::skill_level_events::Source {
                attempt: &receipt,
                chat: &owner.conversation,
                message: sequence,
            }),
        )?;
    }
    db.execute(
        "UPDATE turns SET context=json_remove(context,?2) WHERE id=?1",
        params![owner.turn, format!("$.{kind}Error")],
    )?;
    db.execute(
        "UPDATE conversations SET revision=revision+1 WHERE id=?1",
        [owner.conversation],
    )?;
    super::super::bump(db)?;
    Ok(true)
}
