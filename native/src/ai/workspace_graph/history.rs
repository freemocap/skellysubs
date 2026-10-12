//! Read persisted native facts without loading executable capabilities.
use super::*;
use crate::{ai::workspace_graph::error, model::Result as AppResult};
use rusqlite::params;

pub fn inspection(db: &Connection, engine: &str) -> AppResult<HistoricalInspection> {
    let partition = db.query_row(
        "SELECT workspace_id,catalog,1 FROM workspace_graph_engines WHERE id=?1 UNION ALL SELECT conversation_id,catalog,0 FROM graph_engines WHERE id=?1",
        [engine],
        |r| {
            Ok(Partition {
                owner: if r.get::<_,bool>(2)? { Owner::Workspace(r.get(0)?) } else { Owner::Conversation(r.get(0)?) },
                catalog: r.get(1)?,
            })
        },
    )?;
    let tx = db.unchecked_transaction()?;
    let mut reader = graph_store::BorrowedReadStore::new(&tx, partition).map_err(error)?;
    let checkpoint = reader
        .checkpoint(limits().checkpoint)
        .map_err(error)?
        .ok_or_else(|| error(fault()))?;
    checkpoint
        .historical_inspection(
            checkpoint.stamp().revision,
            HistoricalLimits {
                history: limits().history,
                state: limits().state,
            },
            &mut reader,
        )
        .map_err(error)
}

pub fn usage(
    db: &Connection,
    language: Option<&str>,
    persona: Option<&str>,
) -> AppResult<crate::conversations::execution::graph_runtime::Usage> {
    usage_for_kind(db, language, persona, None)
}
pub fn usage_for_kind(
    db: &Connection,
    language: Option<&str>,
    persona: Option<&str>,
    kind: Option<&str>,
) -> AppResult<crate::conversations::execution::graph_runtime::Usage> {
    let mut total = crate::conversations::execution::graph_runtime::Usage::default();
    let engines = db.prepare("SELECT DISTINCT e.id FROM workspace_graph_engines e JOIN workspace_graph_transport_identities w ON w.engine_id=e.id")?
        .query_map([], |r| r.get::<_, String>(0))?.collect::<rusqlite::Result<Vec<_>>>()?;
    for engine in engines {
        let history = inspection(db, &engine)?;
        let executions = db
            .prepare(
                "SELECT execution_id FROM workspace_graph_transport_identities WHERE engine_id=?1",
            )?
            .query_map([&engine], |r| r.get::<_, String>(0))?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        // Shared speech captures provider language tags, not product language
        // identities. Attribute it through its reading consumers, once per scope.
        let runs = db.prepare("SELECT run_id FROM workspace_graph_runs r WHERE engine_id=?1 AND (?3 IS NULL OR kind=?3) AND ((?4 IS NULL AND (?2 IS NULL OR json_extract(context,'$.scope.language')=?2)) OR EXISTS(SELECT 1 FROM workspace_graph_consumers u LEFT JOIN reading_attempts reading ON reading.id=u.consumer_id LEFT JOIN transcription_attempts transcription ON transcription.id=u.consumer_id LEFT JOIN conversations c ON c.id=transcription.conversation_id LEFT JOIN contacts contact ON contact.id=c.contact_id LEFT JOIN drill_items d ON d.id=transcription.drill_item_id WHERE u.run_id=r.run_id AND (?2 IS NULL OR COALESCE(c.language_id,d.language_id,json_extract(reading.receipt,'$.language'))=?2) AND (?4 IS NULL OR contact.persona_id=?4)))")?
            .query_map(params![engine, language, kind, persona], |r| r.get::<_,String>(0))?.collect::<rusqlite::Result<Vec<_>>>()?;
        let mut included = std::collections::BTreeSet::new();
        for run in runs {
            let view = history
                .snapshot(
                    &run,
                    ExportLimits {
                        bytes: 32 * 1024 * 1024,
                        attempts: 32768,
                    },
                )
                .map_err(error)?;
            for attempts in view.attempts.into_values() {
                for attempt in attempts {
                    included.insert(attempt.execution);
                }
            }
        }
        for execution in executions {
            let id: ExecutionId = serde_json::from_str(&execution)?;
            if !included.contains(&execution) {
                continue;
            }
            total.attempts += 1;
            let evidence = history.execution_evidence(id).map_err(error)?;
            let (mut input, mut output) = (None, None);
            for observation in evidence.into_iter().flat_map(|e| e.observations) {
                if let Some(usage) = observation.usage {
                    if usage.input_tokens.is_some() {
                        input = usage.input_tokens;
                    }
                    if usage.output_tokens.is_some() {
                        output = usage.output_tokens;
                    }
                }
            }
            if input.is_none() || output.is_none() {
                total.unknown += 1;
            }
            let add = |current: i32, count: Option<u64>| -> AppResult<i32> {
                count
                    .unwrap_or(0)
                    .try_into()
                    .ok()
                    .and_then(|v| current.checked_add(v))
                    .ok_or_else(|| {
                        crate::model::AppError::new(
                            crate::model::ErrorCode::Storage,
                            "Graph usage exceeds the report's integer range.",
                        )
                    })
            };
            total.input = add(total.input, input)?;
            total.output = add(total.output, output)?;
        }
    }
    Ok(total)
}

pub fn receipt(db: &Connection, run: &str) -> AppResult<Option<serde_json::Value>> {
    use rusqlite::OptionalExtension;
    let engine: Option<String> = db
        .query_row(
            "SELECT engine_id FROM workspace_graph_runs WHERE run_id=?1",
            [run],
            |r| r.get(0),
        )
        .optional()?;
    let Some(engine) = engine else {
        return Ok(None);
    };
    let history = inspection(db, &engine)?;
    receipt_from_history(&history, &engine, run)
}

pub(crate) fn receipt_from_history(
    history: &HistoricalInspection,
    engine: &str,
    run: &str,
) -> AppResult<Option<serde_json::Value>> {
    let view = history
        .snapshot(
            run,
            ExportLimits {
                bytes: 32 * 1024 * 1024,
                attempts: 32768,
            },
        )
        .map_err(error)?;
    let attempt = view
        .attempts
        .iter()
        .filter(|(node, _)| {
            view.artifact
                .definition
                .nodes
                .get(*node)
                .and_then(|n| view.artifact.operations.get(&n.operation))
                .is_some_and(|o| o.resource == Resource::Provider)
        })
        .filter_map(|(_, a)| a.last())
        .next_back();
    let Some(attempt) = attempt else {
        let state = if view.nodes.values().any(|s| *s == Disposition::Cancelled) {
            "cancelled"
        } else {
            "held"
        };
        return Ok(Some(
            serde_json::json!({"nativeEngine":engine,"nativeRun":run,"state":state,"dispatched":false}),
        ));
    };
    let mut receipt =
        execution_receipt_from_history(history, engine, serde_json::from_str(&attempt.execution)?)?;
    receipt["nativeRun"] = serde_json::json!(run);
    Ok(Some(receipt))
}

pub fn execution_receipt(
    db: &Connection,
    engine: &str,
    execution: ExecutionId,
) -> AppResult<serde_json::Value> {
    let history = inspection(db, engine)?;
    execution_receipt_from_history(&history, engine, execution)
}

pub(crate) fn execution_receipt_from_history(
    history: &HistoricalInspection,
    engine: &str,
    execution: ExecutionId,
) -> AppResult<serde_json::Value> {
    let producer = history.producer_receipt(execution).map_err(error)?;
    let evidence = history.execution_evidence(execution).map_err(error)?;
    let mut state = match producer.outcome {
        Some(Ok(())) => "succeeded",
        Some(Err(_)) => "failed",
        None if producer.unknown => "unknown",
        None => "pending",
    };
    let mut response = serde_json::json!({"nativeEvidence":evidence});
    for observation in evidence.into_iter().flat_map(|e| e.observations) {
        if observation.error_code.as_deref() == Some("unknown_outcome") {
            state = "unknown";
        }
        if let Some(EvidenceValue::ClassifiedJson(transport)) =
            observation.additional.get("transport")
        {
            if let Some(details) = transport.get("diagnostics") {
                response["diagnostics"] = details.clone();
            }
            if let Some(cost) = transport.get("cost_micros") {
                response["costMicros"] = cost.clone();
            }
        }

        if let Some(value) = observation.request_id {
            response["providerId"] = serde_json::json!(value);
        }
        if let Some(value) = observation.actual_model {
            response["actualModel"] = serde_json::json!(value);
        }
        if let Some(value) = observation.finish_reason {
            response["finishReason"] = serde_json::json!(value);
        }
        if let Some(value) = observation.usage {
            response["inputTokens"] = serde_json::json!(value.input_tokens);
            response["outputTokens"] = serde_json::json!(value.output_tokens);
        }
    }
    Ok(
        serde_json::json!({"nativeEngine":engine,"nativeExecution":execution,"state":state,"dispatched":producer.dispatched,"response":response}),
    )
}
