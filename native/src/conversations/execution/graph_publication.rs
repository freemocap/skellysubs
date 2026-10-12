//! Conversation publication inside the native graph's owner transaction.
//! These adapters do not schedule work or commit. The host must retain any
//! returned AppError (including diagnostics) when rejecting a graph commit.
use crate::ai::graph::{Authority, CommitIntent, CommitRequest};
use crate::model::{AppError, ErrorCode, Result};
use rusqlite::{Connection, params};
mod audio;
mod helpers;
mod surfaces;
pub use audio::adopt_audio;
pub(crate) use surfaces::surfaces;
mod source;
pub use helpers::publish_helper;
mod learning;
pub use learning::publish_assessment;

#[cfg(test)]
mod tests;

fn transaction(db: &Connection) -> Result<()> {
    if db.is_autocommit() {
        return Err(AppError::new(
            ErrorCode::Conflict,
            "Graph publication requires the owner transaction.",
        ));
    }
    Ok(())
}

fn rejected() -> AppError {
    AppError::new(
        ErrorCode::Conflict,
        "Graph publication authority does not match its owner.",
    )
}

/// Declare the reply effect once with Begin, after staging its graph turn owner.
/// Node and output port come from the executable's domain binding. Captured
/// attribution and scope never follow later settings changes or retry identities.
pub fn declare_reply(
    db: &Connection,
    request: &CommitRequest<'_>,
    node: &str,
    output_port: &str,
) -> Result<String> {
    transaction(db)?;
    let CommitIntent::Begin { authority, inputs } = &request.intent else {
        return Err(rejected());
    };
    let definition = request
        .next
        .inspection_definition(
            authority.artifact,
            crate::ai::graph::ExportLimits {
                bytes: 4 * 1024 * 1024,
                attempts: 0,
            },
        )
        .map_err(super::graph_runtime::error)?;
    let output = definition
        .artifact
        .definition
        .nodes
        .get(node)
        .and_then(|node| definition.artifact.operations.get(&node.operation))
        .and_then(|operation| operation.outputs.get(output_port));
    if !output.is_some_and(|port| !port.optional && port.contract == super::prose::text_contract())
    {
        return Err(AppError::new(
            ErrorCode::Validation,
            "Reply effect must bind a required generated-text output in its executable artifact.",
        ));
    }
    let (turn, language, raw, role): (String, String, String, String) = db.query_row(
        "SELECT t.id,c.language_id,t.context,CASE o.channel WHEN 'coach' THEN 'coach_reply' ELSE o.channel END FROM turn_execution_owners o JOIN turns t ON t.id=o.turn_id JOIN conversations c ON c.id=t.conversation_id WHERE o.executor='graph' AND o.channel IN ('coach','persona_reply','persona_opening') AND o.engine_id=?1 AND o.run_id=?2 AND o.artifact_id=?3",
        params![request.next.stamp().engine, authority.run, authority.artifact],
        |r| Ok((r.get(0)?,r.get(1)?,r.get(2)?,r.get(3)?)),
    )?;
    let context: serde_json::Value = serde_json::from_str(&raw)?;
    let variety = context["practiceSettings"]["varietyId"]
        .as_str()
        .filter(|v| !v.is_empty())
        .ok_or_else(|| {
            AppError::new(
                ErrorCode::Validation,
                "Missing graph effect source variety.",
            )
        })?;
    let reserved_source = source::reserved_source(&definition.artifact, inputs, node, output_port)?;
    let effect = uuid::Uuid::new_v4().to_string();
    db.execute(
        "INSERT INTO conversation_graph_effects(id,turn_id,node_key,output_port,role,scope,language_id,variety_id,award_source) VALUES(?1,?2,?3,?4,?9,?5,?6,?7,?8)",
        params![effect,turn,node,output_port,authority.scope,language,variety,format!("graph-effect:{effect}"),role],
    )?;
    if let Some(message) = reserved_source {
        db.execute(
            "INSERT INTO conversation_graph_reply_sources(effect_id,message_id) VALUES(?1,?2)",
            params![effect, message],
        )?;
    }
    Ok(effect)
}

/// Publish only native Adopt values. The required authority callback rechecks
/// current access/configuration in this same transaction; captured scope alone
/// is never proof of current permission. The caller rolls back on every error.
/// Native graph facts own the turn-state projection.
pub fn publish_reply(
    db: &Connection,
    request: &CommitRequest<'_>,
    authorize: impl FnOnce(&Connection, &Authority<'_>) -> Result<()>,
) -> Result<()> {
    transaction(db)?;
    let CommitIntent::Adopt {
        authority,
        attempt,
        execution,
        values,
    } = &request.intent
    else {
        return Err(rejected());
    };
    let (effect, turn, conversation, output_port, role): (String, String, String, String, String) = db.query_row(
        "SELECT e.id,t.id,t.conversation_id,e.output_port,e.role FROM conversation_graph_effects e JOIN turn_execution_owners o ON o.turn_id=e.turn_id JOIN turns t ON t.id=o.turn_id JOIN conversations c ON c.id=t.conversation_id JOIN contacts contact ON contact.id=c.contact_id WHERE o.executor='graph' AND o.channel IN ('coach','persona_reply','persona_opening') AND o.engine_id=?1 AND o.run_id=?2 AND o.artifact_id=?3 AND e.node_key=?4 AND e.scope=?5 AND e.role=CASE o.channel WHEN 'coach' THEN 'coach_reply' ELSE o.channel END AND c.archived=0 AND contact.archived=0 AND t.state IN ('pending','assisting')",
        params![request.next.stamp().engine,authority.run,authority.artifact,authority.node,authority.scope],
        |r| Ok((r.get(0)?,r.get(1)?,r.get(2)?,r.get(3)?,r.get(4)?)),
    )?;
    authorize(db, authority)?;
    let text = values.get(&output_port).and_then(serde_json::Value::as_str)
        .ok_or_else(|| AppError::new(ErrorCode::Validation,"Graph reply output must be text.")
            .with_diagnostics(serde_json::json!({"stage":"graph_publication","path":output_port,"expected":"string"})))?;
    crate::conversations::reply_contract::validate(text)?;
    crate::conversations::phrase_start::validate(db, &turn, text)?;
    let message = source::message_id(db, &effect)?;
    db.execute("INSERT INTO messages(id,conversation_id,turn_id,sequence,role,text) SELECT ?1,?2,?3,COALESCE(MAX(sequence),0)+1,'assistant',?4 FROM messages WHERE conversation_id=?2",
        params![message,conversation,turn,text])?;
    db.execute("INSERT INTO conversation_graph_publications(effect_id,attempt_id,execution_id,message_id) VALUES(?1,?2,?3,?4)",
        params![effect,serde_json::to_string(attempt)?,serde_json::to_string(execution)?,message])?;
    if role == "coach_reply" {
        crate::learning::effort::exploration::graph_coach_reply(db, &effect)?;
    } else {
        // The saved partner reply is the playback source. Publish its exact
        // identity/text atomically with the message, just as the conversation
        // projection expects; never reconstruct it from a later reply attempt.
        db.execute(
            "UPDATE turns SET context=json_set(context,'$.speechSourceId',?2,'$.speechSourceText',?3) WHERE id=?1",
            params![turn, message, text],
        )?;
    }
    db.execute(
        "UPDATE conversations SET revision=revision+1 WHERE id=?1",
        [conversation],
    )?;
    super::bump(db)?;
    Ok(())
}
