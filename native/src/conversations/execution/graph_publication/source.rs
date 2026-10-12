use super::*;
use crate::ai::graph::{Artifact, Source, Values};
use rusqlite::OptionalExtension;

/// Follow the executable's actual source-binding operation, not a conventional
/// node/input name. All consumers of this reply must agree on its admitted ID.
pub(super) fn reserved_source<V>(
    artifact: &Artifact<V>,
    inputs: &Values,
    reply: &str,
    port: &str,
) -> Result<Option<String>> {
    let mut reserved = None;
    for node in artifact.definition.nodes.values().filter(|node| {
        node.operation == super::super::reply_reading_graph::source_binding_contract()
    }) {
        if !matches!(node.inputs.get("text"), Some(Source::Output {node,port:output}) if node == reply && output == port)
        {
            continue;
        }
        let Some(Source::Input(input)) = node.inputs.get("id") else {
            return Err(rejected());
        };
        let id = inputs
            .get(input)
            .and_then(serde_json::Value::as_str)
            .filter(|id| !id.is_empty())
            .ok_or_else(rejected)?;
        if reserved.as_deref().is_some_and(|previous| previous != id) {
            return Err(rejected());
        }
        reserved = Some(id.to_owned());
    }
    Ok(reserved)
}
pub(super) fn message_id(db: &Connection, effect: &str) -> Result<String> {
    let reserved: Option<String> = db
        .query_row(
            "SELECT message_id FROM conversation_graph_reply_sources WHERE effect_id=?1",
            [effect],
            |r| r.get(0),
        )
        .optional()?;
    Ok(reserved.unwrap_or_else(|| uuid::Uuid::new_v4().to_string()))
}

pub(super) struct Owner {
    pub turn: String,
    pub conversation: String,
    pub role: String,
    pub node: String,
}

pub(super) fn adopted_owner(
    db: &Connection,
    request: &CommitRequest<'_>,
    work: &crate::ai::graph::Work,
    source: &crate::language::source_graph::SourceText,
    captured: &crate::language::source_graph::SourceText,
) -> Result<Owner> {
    use crate::ai::graph::ExportLimits;
    transaction(db)?;
    if source != captured {
        return Err(rejected());
    }
    let CommitIntent::Adopt {
        authority,
        execution,
        ..
    } = &request.intent
    else {
        return Err(rejected());
    };
    if work.execution != *execution || work.artifact != authority.artifact {
        return Err(rejected());
    }
    let node = authority.node.ok_or_else(rejected)?;
    let definition = request
        .next
        .inspection_definition(
            authority.artifact,
            ExportLimits {
                bytes: 4 * 1024 * 1024,
                attempts: 0,
            },
        )
        .map_err(super::super::graph_runtime::error)?;
    if !definition
        .artifact
        .definition
        .nodes
        .get(node)
        .is_some_and(|node| node.operation == work.operation)
    {
        return Err(rejected());
    }
    let (turn, conversation, role): (String,String,String) = db.query_row(
        "SELECT t.id,t.conversation_id,m.role FROM graph_conversation_runs o JOIN turns t ON t.id=o.turn_id JOIN conversations c ON c.id=t.conversation_id JOIN contacts contact ON contact.id=c.contact_id JOIN messages m ON m.turn_id=t.id WHERE o.channel IN ('persona_reply','persona_opening','speech','helper') AND o.engine_id=?1 AND o.run_id=?2 AND o.artifact_id=?3 AND m.id=?4 AND m.text=?5 AND c.archived=0 AND contact.archived=0 AND t.state NOT IN ('cancelled','invalidated') AND NOT EXISTS(SELECT 1 FROM turns child WHERE child.replaces_turn_id=t.id)",
        params![request.next.stamp().engine,authority.run,authority.artifact,source.id,source.text],
        |r| Ok((r.get(0)?,r.get(1)?,r.get(2)?)))?;

    Ok(Owner {
        turn,
        conversation,
        role,
        node: node.to_owned(),
    })
}
