//! Durable metadata for proposal inference, independent of conversations.
//! Prompts, briefs, credentials and proposal text never enter this table.
use crate::ai::generation::Request;
use crate::ai::transport::provider::Completion;
use crate::model::*;
use crate::storage::store::Store;
use rusqlite::{Connection, OptionalExtension, params};

fn changed(db: &Connection) -> Result<()> {
    db.execute("UPDATE metadata SET revision=revision+1", [])?;
    Ok(())
}
fn unavailable() -> AppError {
    AppError::new(
        ErrorCode::Conflict,
        "This persona generation receipt is no longer active.",
    )
}

pub fn begin(store: &mut Store, request: &Request) -> Result<()> {
    request.validate(store)?;
    let tx = store.connection.transaction()?;
    tx.execute("INSERT INTO generation_attempts(id,attempt_id,operation_id,language_id,route,requested_model,profile_revision,state,kind) VALUES(?1,?2,?3,?4,?5,?6,?7,'pending',?8)",
        params![request.id,request.attempt,request.operation,request.language_id,request.target.route.label(),request.target.model,request.target.revision,request.kind])?;
    changed(&tx)?;
    tx.commit()?;
    Ok(())
}

pub fn dispatch(store: &mut Store, request: &Request) -> Result<()> {
    request.validate(store)?;
    let tx = store.connection.transaction()?;
    if tx.execute("UPDATE generation_attempts SET state='running',dispatched_at=strftime('%Y-%m-%dT%H:%M:%fZ','now') WHERE id=?1 AND attempt_id=?2 AND state='pending'", params![request.id,request.attempt])? != 1 {
        return Err(unavailable());
    }
    changed(&tx)?;
    tx.commit()?;
    Ok(())
}

fn stop(store: &mut Store, request: &Request, pending_message: &str) -> Result<()> {
    let tx = store.connection.transaction()?;
    let rows = tx.execute("UPDATE generation_attempts SET state=CASE WHEN dispatched_at IS NULL THEN 'cancelled' ELSE 'unknown' END,error=CASE WHEN dispatched_at IS NULL THEN ?3 ELSE 'Generation stopped locally; provider outcome and billing may be unknown. No automatic retry was made.' END,finished_at=strftime('%Y-%m-%dT%H:%M:%fZ','now') WHERE id=?1 AND attempt_id=?2 AND state IN ('pending','running')", params![request.id,request.attempt,pending_message])?;
    if rows != 0 {
        changed(&tx)?;
    }
    tx.commit()?;
    Ok(())
}
pub fn cancel(store: &mut Store, request: &Request) -> Result<()> {
    stop(store, request, "Generation cancelled before submission.")
}
pub fn expire(store: &mut Store, request: &Request) -> Result<()> {
    stop(
        store,
        request,
        "Generation ownership expired before submission.",
    )
}

fn safe_error(error: &AppError) -> &str {
    match error.code {
        // Persona validation can mention the rejected symbol, which belongs to
        // the volatile proposal, not metadata-only receipts.
        ErrorCode::Validation => "The generated persona failed validation.",
        ErrorCode::Storage => "Local storage prevented generation from completing.",
        ErrorCode::Credential => "Credential access failed.",
        ErrorCode::Internal => "Generation failed because of an internal error.",
        _ => &error.message,
    }
}

pub fn finish<T>(
    store: &mut Store,
    request: &Request,
    completion: Option<&Completion>,
    outcome: &Result<T>,
) -> Result<()> {
    let tx = store.connection.transaction()?;
    let stopped = finish_in(&tx, request, completion, outcome)?;
    if stopped.is_none() && outcome.is_ok() && request.kind == "persona" {
        crate::learning::effort::bot::persona(&tx, request)?;
    }
    tx.commit()?;
    match stopped {
        Some(error) => Err(error),
        None => Ok(()),
    }
}

pub(crate) fn finish_in<T>(
    tx: &Connection,
    request: &Request,
    completion: Option<&Completion>,
    outcome: &Result<T>,
) -> Result<Option<AppError>> {
    let state: Option<String> = tx
        .query_row(
            "SELECT state FROM generation_attempts WHERE id=?1 AND attempt_id=?2",
            params![request.id, request.attempt],
            |r| r.get(0),
        )
        .optional()?;
    let state = state.ok_or_else(unavailable)?;
    let stopped = state == "cancelled" || state == "unknown";
    if !stopped && state != "pending" && state != "running" {
        return Err(unavailable());
    }
    if outcome.is_ok() && state == "pending" {
        return Err(unavailable());
    }
    let terminal = if stopped {
        state.as_str()
    } else {
        match outcome {
            Ok(_) => "succeeded",
            Err(error) if error.code == ErrorCode::UnknownOutcome => "unknown",
            Err(_) => "failed",
        }
    };
    let error = outcome.as_ref().err().map(|error| {
        if request.kind == "drill" {
            crate::diagnostics::response::scrub(
                &error.message,
                &[
                    request.brief.as_deref().unwrap_or_default(),
                    completion.map(|c| c.text.as_str()).unwrap_or_default(),
                ],
            )
        } else {
            safe_error(error).to_owned()
        }
    });
    let diagnostics = crate::diagnostics::response::retained_with_private(
        completion.and_then(|c| c.diagnostics.as_ref()),
        outcome.as_ref().err(),
        &[
            request.credential.as_str(),
            request.brief.as_deref().unwrap_or_default(),
            completion.map(|c| c.text.as_str()).unwrap_or_default(),
        ],
    );
    tx.execute(
        "UPDATE generation_attempts SET diagnostics=?2 WHERE id=?1",
        params![request.id, diagnostics],
    )?;
    tx.execute("UPDATE generation_attempts SET state=?2,finished_at=COALESCE(finished_at,strftime('%Y-%m-%dT%H:%M:%fZ','now')),actual_model=COALESCE(actual_model,?3),provider_id=COALESCE(provider_id,?4),input_tokens=COALESCE(input_tokens,?5),output_tokens=COALESCE(output_tokens,?6),error=CASE WHEN ?7 THEN error ELSE ?8 END,finish_reason=COALESCE(finish_reason,?9) WHERE id=?1",
        params![request.id,terminal,completion.map(|c| c.actual_model.as_str()),completion.map(|c| c.provider_id.as_str()),completion.and_then(|c|c.input_tokens),completion.and_then(|c|c.output_tokens),stopped,error,completion.map(|c|c.finish_reason.as_str())])?;
    changed(tx)?;
    if stopped && outcome.is_ok() {
        return Ok(Some(AppError::new(
            if state == "unknown" {
                ErrorCode::UnknownOutcome
            } else {
                ErrorCode::Conflict
            },
            "This generation was stopped. Its proposal cannot be adopted.",
        )));
    }
    Ok(None)
}

pub fn recover(db: &Connection) -> Result<()> {
    let tx = db.unchecked_transaction()?;
    let rows = tx.execute("UPDATE generation_attempts SET state=CASE WHEN dispatched_at IS NULL THEN 'cancelled' ELSE 'unknown' END,finished_at=strftime('%Y-%m-%dT%H:%M:%fZ','now'),error=CASE WHEN dispatched_at IS NULL THEN 'Generation interrupted before submission.' ELSE 'Generation interrupted; provider outcome and billing may be unknown. No automatic retry was made.' END WHERE state IN ('pending','running')", [])?;
    if rows != 0 {
        changed(&tx)?;
    }
    tx.commit()?;
    Ok(())
}

fn usage_for(db: &Connection, kind: &str) -> Result<PersonaGenerationUsage> {
    let usage = crate::ai::workspace_graph::usage_for_kind(db, None, None, Some(kind))?;
    Ok(PersonaGenerationUsage {
        attempts: usage.attempts,
        input_tokens: usage.input,
        output_tokens: usage.output,
        unknown_usage: usage.unknown,
    })
}

pub fn activity(db: &Connection) -> Result<PersonaGenerationActivity> {
    activity_for(db, "persona")
}
fn source_field<T: serde::de::DeserializeOwned>(
    response: &serde_json::Value,
    field: &str,
) -> Result<Option<T>> {
    Ok(serde_json::from_value(response[field].clone())?)
}
pub(crate) fn activity_for(db: &Connection, kind: &str) -> Result<PersonaGenerationActivity> {
    let mut query = db.prepare("SELECT id,attempt_id,operation_id,language_id,route,requested_model,profile_revision,state,created_at,dispatched_at,finished_at,actual_model,provider_id,input_tokens,output_tokens,error,diagnostics,finish_reason FROM generation_attempts WHERE kind=?1 ORDER BY rowid DESC LIMIT 50")?;
    let attempts = query
        .query_map([kind], |r| {
            let route: String = r.get(4)?;
            Ok((
                route,
                PersonaGenerationAttempt {
                    finish_reason: r.get(17)?,
                    diagnostics: crate::diagnostics::response::column(r, 16)?,
                    id: r.get(0)?,
                    attempt_id: r.get(1)?,
                    operation_id: r.get(2)?,
                    language_id: r.get(3)?,
                    route: ConnectionRoute::Hosted,
                    requested_model: r.get(5)?,
                    profile_revision: r.get(6)?,
                    state: r.get(7)?,
                    created_at: r.get(8)?,
                    dispatched_at: r.get(9)?,
                    finished_at: r.get(10)?,
                    actual_model: r.get(11)?,
                    provider_id: r.get(12)?,
                    input_tokens: r.get(13)?,
                    output_tokens: r.get(14)?,
                    error: r.get(15)?,
                },
            ))
        })?
        .map(|row| {
            let (route, mut view) = row?;
            view.route = ConnectionRoute::parse(&route)?;
            if let Some(source) = crate::ai::workspace_graph::receipt(db, &view.id)? {
                let response = &source["response"];
                view.actual_model = view.actual_model.or(source_field(response, "actualModel")?);
                view.provider_id = view.provider_id.or(source_field(response, "providerId")?);
                view.finish_reason = view
                    .finish_reason
                    .or(source_field(response, "finishReason")?);
                view.input_tokens = view.input_tokens.or(source_field(response, "inputTokens")?);
                view.output_tokens = view
                    .output_tokens
                    .or(source_field(response, "outputTokens")?);
                let mut diagnostics = view
                    .diagnostics
                    .take()
                    .unwrap_or_else(|| serde_json::json!({}));
                diagnostics["sourceExecution"] = source;
                view.diagnostics = Some(diagnostics);
            }
            Ok(view)
        })
        .collect::<Result<Vec<_>>>()?;
    Ok(PersonaGenerationActivity {
        revision: db.query_row("SELECT revision FROM metadata", [], |r| r.get(0))?,
        attempts,
        usage: usage_for(db, kind)?,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    fn setup() -> (tempfile::TempDir, Store) {
        let dir = tempfile::tempdir().unwrap();
        let mut store = Store::open(&dir.path().join("workspace.sqlite3")).unwrap();
        store.prepare_chat().unwrap();
        store.connection.execute("UPDATE ai_config SET route='custom',custom_config=json_set(custom_config,'$.baseUrl','http://127.0.0.1:8765/v1','$.bearerAuth',json('false'))", []).unwrap();
        (dir, store)
    }
    fn request(store: &Store, language: &str) -> Request {
        Request::capture(
            store,
            language.into(),
            Some("PRIVATE-BRIEF-SENTINEL".into()),
        )
        .unwrap()
    }
    fn completed(input: Option<i32>, output: Option<i32>) -> Completion {
        Completion {
            diagnostics: None,
            text: "PRIVATE-PROPOSAL-SENTINEL".into(),
            finish_reason: "stop".into(),
            actual_model: "reported-model".into(),
            provider_id: "reported-request".into(),
            input_tokens: input,
            output_tokens: output,
        }
    }
    fn details(store: &Store) -> PersonaDetails {
        store.snapshot().unwrap().personas[0].details.clone()
    }
    fn submit(store: &mut Store, request: &Request) {
        begin(store, request).unwrap();
        dispatch(store, request).unwrap();
        request.mark_submitted();
    }

    #[test]
    fn successful_receipt_is_durable_metadata_and_usage_has_correct_ownership() {
        let (dir, mut store) = setup();
        let request = request(&store, "spanish");
        submit(&mut store, &request);
        let outcome = Ok(details(&store));
        finish::<PersonaDetails>(
            &mut store,
            &request,
            Some(&completed(Some(12), Some(7))),
            &outcome,
        )
        .unwrap();
        assert!(finish::<PersonaDetails>(&mut store, &request, None, &outcome).is_err());
        let effort = crate::learning::effort::read(&store.connection, "spanish").unwrap();
        assert_eq!(effort.bot, 1);
        assert_eq!(effort.explorations, 0);
        assert!(effort.recent[0].conversation_id.is_none());
        // A domain receipt alone is not a dispatched graph execution.
        assert_eq!(store.profile().unwrap().global.attempts, 0);
        drop(store);
        let store = Store::open(&dir.path().join("workspace.sqlite3")).unwrap();
        let view = activity(&store.connection).unwrap();
        assert_eq!(view.attempts[0].state, "succeeded");
        assert_eq!(
            view.attempts[0].actual_model.as_deref(),
            Some("reported-model")
        );
        assert!(view.attempts[0].dispatched_at.is_some());
        assert!(view.attempts[0].finished_at.is_some());
        let json = serde_json::to_string(&view).unwrap();
        assert!(!json.contains("PRIVATE-"));
        assert_eq!(view.usage.attempts, 0);
    }

    #[test]
    fn cancellations_do_not_invent_usage_or_allow_late_success() {
        let (_dir, mut store) = setup();
        let pending = request(&store, "spanish");
        begin(&mut store, &pending).unwrap();
        cancel(&mut store, &pending).unwrap();
        cancel(&mut store, &pending).unwrap();
        assert!(dispatch(&mut store, &pending).is_err());
        assert_eq!(usage_for(&store.connection, "persona").unwrap().attempts, 0);
        let running = request(&store, "spanish");
        submit(&mut store, &running);
        cancel(&mut store, &running).unwrap();
        let outcome = Ok(details(&store));
        let error = finish::<PersonaDetails>(
            &mut store,
            &running,
            Some(&completed(Some(5), None)),
            &outcome,
        )
        .unwrap_err();
        assert_eq!(error.code, ErrorCode::UnknownOutcome);
        assert_eq!(
            crate::learning::effort::read(&store.connection, "spanish")
                .unwrap()
                .bot,
            0
        );
        let view = activity(&store.connection).unwrap();
        assert_eq!(view.attempts[0].state, "unknown");
        assert_eq!(view.attempts[1].state, "cancelled");
        assert_eq!(view.usage.attempts, 0);
        assert_eq!(view.attempts[0].input_tokens, Some(5));
        assert_eq!(view.attempts[0].output_tokens, None);
        assert_eq!(view.usage.unknown_usage, 0);
        // A second terminal callback cannot replace already reported metadata.
        let mut duplicate = completed(Some(500), Some(9));
        duplicate.actual_model = "different-model".into();
        assert!(
            finish::<PersonaDetails>(&mut store, &running, Some(&duplicate), &outcome).is_err()
        );
        let view = activity(&store.connection).unwrap();
        assert_eq!(view.attempts[0].input_tokens, Some(5));
        assert_eq!(view.attempts[0].output_tokens, Some(9));
        assert_eq!(
            view.attempts[0].actual_model.as_deref(),
            Some("reported-model")
        );
    }

    #[test]
    fn rejected_proposals_retain_reported_usage_without_retaining_rejected_text() {
        let (_dir, mut store) = setup();
        let request = request(&store, "spanish");
        submit(&mut store, &request);
        let outcome = Err(AppError::new(
            ErrorCode::Validation,
            "Invalid Vibe: PRIVATE-PROPOSAL-SENTINEL",
        ));
        finish::<PersonaDetails>(
            &mut store,
            &request,
            Some(&completed(Some(3), Some(4))),
            &outcome,
        )
        .unwrap();
        let view = activity(&store.connection).unwrap();
        assert_eq!(view.attempts[0].state, "failed");
        assert_eq!(
            crate::learning::effort::read(&store.connection, "spanish")
                .unwrap()
                .bot,
            0
        );
        assert_eq!(view.attempts[0].input_tokens, Some(3));
        assert_eq!(view.attempts[0].output_tokens, Some(4));
        assert_eq!(view.usage.unknown_usage, 0);
        assert!(!serde_json::to_string(&view).unwrap().contains("PRIVATE-"));
        assert!(
            serde_json::to_string(&view)
                .unwrap()
                .contains("Invalid Vibe:")
        );
    }

    #[test]
    fn restart_settles_pending_and_running_without_replay_and_is_idempotent() {
        let (dir, mut store) = setup();
        let pending = request(&store, "spanish");
        begin(&mut store, &pending).unwrap();
        let running = request(&store, "french");
        submit(&mut store, &running);
        drop(store);
        let store = Store::open(&dir.path().join("workspace.sqlite3")).unwrap();
        let view = activity(&store.connection).unwrap();
        assert_eq!(view.attempts[0].state, "unknown");
        assert_eq!(view.attempts[1].state, "cancelled");
        assert_eq!(view.usage.attempts, 0);
        assert_eq!(view.usage.unknown_usage, 0);
        let rows = serde_json::to_string(&view.attempts).unwrap();
        drop(store);
        let store = Store::open(&dir.path().join("workspace.sqlite3")).unwrap();
        assert_eq!(
            serde_json::to_string(&activity(&store.connection).unwrap().attempts).unwrap(),
            rows
        );
        assert_eq!(store.profile().unwrap().global.attempts, 0);
    }

    #[test]
    fn bounded_recent_view_does_not_truncate_totals_and_terminal_write_failure_is_atomic() {
        let (_dir, mut store) = setup();
        for _ in 0..55 {
            let request = request(&store, "spanish");
            submit(&mut store, &request);
            finish::<PersonaDetails>(
                &mut store,
                &request,
                None,
                &Err(AppError::new(
                    ErrorCode::Provider,
                    "Service refused the request.",
                )),
            )
            .unwrap();
        }
        let view = activity(&store.connection).unwrap();
        assert_eq!(view.attempts.len(), 50);
        assert_eq!(view.usage.attempts, 0);
        assert_eq!(view.usage.unknown_usage, 0);
        let request = request(&store, "spanish");
        submit(&mut store, &request);
        let revision = store.snapshot().unwrap().revision;
        store.connection.execute_batch("CREATE TRIGGER refuse_generation_finish BEFORE UPDATE OF finished_at ON generation_attempts BEGIN SELECT RAISE(ABORT,'synthetic write failure'); END;").unwrap();
        let outcome = Ok(details(&store));
        assert!(
            finish::<PersonaDetails>(
                &mut store,
                &request,
                Some(&completed(Some(1), Some(1))),
                &outcome
            )
            .is_err()
        );
        let view = activity(&store.connection).unwrap();
        assert_eq!(view.revision, revision);
        assert_eq!(view.attempts[0].state, "running");
        assert_eq!(view.attempts[0].input_tokens, None);
    }

    #[test]
    fn provider_refusal_survives_restart_without_blocking_explicit_generation() {
        let (dir, mut store) = setup();
        let captured = request(&store, "spanish");
        submit(&mut store, &captured);
        let refusal = AppError::new(ErrorCode::Provider, "Synthetic rate limit refusal")
            .with_refusal(crate::ai::policy::refusal::classify(None, Some(0), None));
        let provider_result = Err(refusal.clone());
        let error =
            crate::ai::generation::accept_completion(&mut store, &captured, &provider_result)
                .unwrap_err();
        assert_eq!(error.message, refusal.message);
        finish::<PersonaDetails>(&mut store, &captured, None, &Err(error)).unwrap();
        Request::capture(&store, "spanish".into(), None).unwrap();
        drop(store);
        let mut store = Store::open(&dir.path().join("workspace.sqlite3")).unwrap();
        let view = activity(&store.connection).unwrap();
        assert_eq!(view.attempts[0].state, "failed");
        assert_eq!(
            view.attempts[0].error.as_deref(),
            Some(refusal.message.as_str())
        );
        assert_eq!(view.usage.attempts, 0);
        assert_eq!(view.usage.unknown_usage, 0);
        let next = Request::capture(&store, "spanish".into(), None).unwrap();
        begin(&mut store, &next).unwrap();
        let view = activity(&store.connection).unwrap();
        assert_eq!(view.attempts[0].state, "pending");
        assert_eq!(view.attempts[1].state, "failed");
        assert_eq!(view.usage.attempts, 0); // Capturing new work never dispatches the failed attempt.
    }
}
