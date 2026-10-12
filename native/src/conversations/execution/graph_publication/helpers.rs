//! Domain projections of adopted source-bound results. Operation identities come
//! from the executable; message roles select display fields, never computation.
use super::*;
use crate::{
    ai::graph::{Contract, Work},
    language::{gloss_graph, source_graph::SourceText, translation_graph},
    learning::coaching::{attribution_graph, conversation_support, support_graph},
};
use serde_json::Value;

enum Projection {
    Translation,
    Gloss,
    Attribution,
    Support(&'static str),
}
fn projection(operation: &Contract) -> Option<(Projection, &'static str)> {
    if *operation == translation_graph::operation_contract() {
        Some((Projection::Translation, "translation"))
    } else if *operation == gloss_graph::operation_contract() {
        Some((Projection::Gloss, "gloss"))
    } else if *operation == attribution_graph::operation_contract() {
        Some((Projection::Attribution, "attribution"))
    } else if *operation == support_graph::Task::Brief.operation() {
        Some((Projection::Support(conversation_support::BRIEF), "support"))
    } else if *operation == support_graph::Task::Assistance.operation() {
        Some((
            Projection::Support(conversation_support::ASSISTANCE),
            "support",
        ))
    } else if *operation == support_graph::explanation::operation_contract() {
        Some((
            Projection::Support(conversation_support::EXPLANATIONS),
            "support",
        ))
    } else {
        None
    }
}

/// Product fields served by the same adapters used for publication. This is a
/// structural association, not a claim about the producing attempt of a value.
pub(super) fn surface(operation: &Contract, port: &str) -> Option<&'static str> {
    let (projection, output) = projection(operation)?;
    if output != port {
        return None;
    }
    Some(match projection {
        Projection::Translation => "translation",
        Projection::Gloss => "gloss",
        Projection::Attribution => "attribution",
        Projection::Support(kind) => kind,
    })
}

/// `work` is the original producer read through DurableEngine's integrity-checked
/// record API. The callback checks current access and all captured secondary
/// sources. Unsupported operations return false so their owning adapter can act.
/// All writes borrow the native adoption transaction and roll back with it.
pub fn publish_helper(
    db: &Connection,
    request: &CommitRequest<'_>,
    work: &Work,
    authorize: impl FnOnce(&Connection, &Authority<'_>) -> Result<()>,
) -> Result<bool> {
    transaction(db)?;
    let Some((projection, port)) = projection(&work.operation) else {
        return Ok(false);
    };
    let CommitIntent::Adopt {
        authority,
        attempt,
        execution: _,
        values,
    } = &request.intent
    else {
        return Err(rejected());
    };
    let bound = values.get(port).ok_or_else(rejected)?;
    let source: SourceText = serde_json::from_value(bound["source"].clone())?;
    let captured = if matches!(projection, Projection::Attribution) {
        &work.inputs["selection"]["source"]
    } else {
        work.inputs.get("source").ok_or_else(rejected)?
    };
    let source::Owner {
        turn,
        conversation,
        role,
        node,
    } = source::adopted_owner(
        db,
        request,
        work,
        &source,
        &serde_json::from_value(captured.clone())?,
    )?;
    authorize(db, authority)?;
    // These remain opaque UI provenance strings. The engine/attempt identities
    // are native and retain exact producing execution identity.
    let operation = format!(
        "graph:{}",
        serde_json::to_string(&(&request.next.stamp().engine, authority.run, node))?
    );
    let attempt = format!(
        "graph:{}",
        serde_json::to_string(&(&request.next.stamp().engine, attempt))?
    );
    let (path, value, error_path) = match projection {
        Projection::Translation => {
            let text = bound["text"].as_str().ok_or_else(rejected)?;
            crate::ai::transport::provider::validate_prose(text)?;
            (
                if role == "user" {
                    "$.userTranslation".into()
                } else {
                    "$.translation".into()
                },
                Value::String(text.into()),
                None,
            )
        }
        Projection::Gloss => {
            let analysis: gloss_graph::result::Analysis = serde_json::from_value(bound.clone())?;
            let mut view = analysis
                .view(&operation, &attempt)
                .map_err(super::super::graph_runtime::error)?;
            let path = if role == "user" {
                "$.userWordGloss"
            } else {
                "$.wordGloss"
            };
            let previous: Option<String> = db.query_row(
                "SELECT json_extract(context,?2) FROM turns WHERE id=?1",
                params![turn, path],
                |r| r.get(0),
            )?;
            if let Some(previous) = previous {
                view =
                    crate::language::gloss::merge_repair(&serde_json::from_str(&previous)?, view)?;
            }
            (
                if role == "user" {
                    "$.userWordGloss".into()
                } else {
                    "$.wordGloss".into()
                },
                serde_json::to_value(view)?,
                Some(
                    if role == "user" {
                        "$.userWordGlossError"
                    } else {
                        "$.wordGlossError"
                    }
                    .to_owned(),
                ),
            )
        }
        Projection::Attribution => {
            if role != "user" {
                return Err(rejected());
            }
            db.execute("UPDATE turns SET context=json_set(context,'$.skillAttributionAttempt',?2) WHERE id=?1",params![turn,attempt])?;
            (
                "$.skillAttribution".into(),
                bound["attribution"].clone(),
                Some("$.skill_attributionError".into()),
            )
        }
        Projection::Support(kind) => {
            if role != "assistant" {
                return Err(rejected());
            }
            (
                format!("$.{kind}"),
                bound["value"].clone(),
                Some(format!("$.{kind}Error")),
            )
        }
    };
    db.execute(
        "UPDATE turns SET context=json_set(context,?2,json(?3)) WHERE id=?1",
        params![turn, path, serde_json::to_string(&value)?],
    )?;
    if let Some(path) = error_path {
        db.execute(
            "UPDATE turns SET context=json_remove(context,?2) WHERE id=?1",
            params![turn, path],
        )?;
    }
    db.execute(
        "UPDATE conversations SET revision=revision+1 WHERE id=?1",
        [conversation],
    )?;
    super::super::bump(db)?;
    Ok(true)
}
