use super::*;
use rusqlite::params;

pub fn begin(store: &Store, request: &Request) -> Result<()> {
    store.connection.execute("INSERT INTO reading_attempts(id,receipt) VALUES(?1,json_set(?2,'$.createdAt',strftime('%Y-%m-%dT%H:%M:%fZ','now')))", params![request.id, serde_json::json!({
        "id":request.id,"attemptId":request.attempt,"operationId":request.operation,"language":request.input.language,
        "kind":request.input.aid.receipt_kind(),"route":request.target.route.label(),
        "requestedModel":request.model,"state":"pending"
    }).to_string()])?;
    Ok(())
}
pub fn cancel(store: &Store, id: &str) -> Result<()> {
    store.connection.execute("UPDATE reading_attempts SET receipt=json_set(receipt,'$.state',CASE WHEN json_extract(receipt,'$.state')='running' THEN 'unknown' ELSE 'cancelled' END,'$.finishedAt',strftime('%Y-%m-%dT%H:%M:%fZ','now')) WHERE id=?1 AND json_extract(receipt,'$.state') IN ('pending','running')", [id])?;
    Ok(())
}
pub fn finish(
    store: &Store,
    request: &Request,
    metadata: serde_json::Value,
    error: Option<&AppError>,
) -> Result<serde_json::Value> {
    let old: String = store.connection.query_row(
        "SELECT receipt FROM reading_attempts WHERE id=?1",
        [&request.id],
        |r| r.get(0),
    )?;
    let mut receipt: serde_json::Value = serde_json::from_str(&old)?;
    let stopped = matches!(receipt["state"].as_str(), Some("unknown" | "cancelled"));
    receipt["response"] = metadata;
    receipt["elapsedMs"] = serde_json::json!(request.created.elapsed().as_millis() as u64);
    if !stopped {
        receipt["state"] = serde_json::json!(if error.is_none() {
            "succeeded"
        } else if error.is_some_and(|e| e.code == ErrorCode::UnknownOutcome) {
            "unknown"
        } else {
            "failed"
        });
    }
    if let Some(error) = error {
        receipt["error"] = serde_json::json!({"code":error.code, "message":crate::diagnostics::response::scrub(&error.message, &[&request.input.text]), "diagnostics":error.diagnostics, "refusal":error.refusal});
    }
    if let Some(award) =
        crate::learning::effort::exploration::reading(&store.connection, request, &receipt)?
    {
        receipt["effortAward"] = serde_json::json!(award);
    }
    store.connection.execute("UPDATE reading_attempts SET receipt=json_set(?2,'$.finishedAt',strftime('%Y-%m-%dT%H:%M:%fZ','now')) WHERE id=?1", params![request.id, receipt.to_string()])?;
    Ok(receipt)
}
pub fn recover(db: &rusqlite::Connection) -> Result<()> {
    db.execute("UPDATE reading_attempts SET receipt=json_set(receipt,'$.state',CASE WHEN json_extract(receipt,'$.state')='running' THEN 'unknown' ELSE 'cancelled' END,'$.interrupted',json('true')) WHERE json_extract(receipt,'$.state') IN ('pending','running')", [])?;
    Ok(())
}
pub fn activity(store: &Store) -> Result<Vec<serde_json::Value>> {
    let mut query = store
        .connection
        .prepare("SELECT receipt FROM reading_attempts ORDER BY rowid DESC LIMIT 50")?;
    query
        .query_map([], |r| r.get::<_, String>(0))?
        .map(|r| {
            let mut receipt: serde_json::Value = serde_json::from_str(&r?)?;
            if let Some(id) = receipt["id"].as_str()
                && let Some(execution) =
                    match crate::ai::workspace_graph::receipt(&store.connection, id)? {
                        Some(receipt) => Some(receipt),
                        None => crate::ai::results::receipt_for_consumer(&store.connection, id)?,
                    }
            {
                receipt["sourceExecution"] = execution;
            }
            Ok(receipt)
        })
        .collect()
}
