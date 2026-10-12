use super::*;

#[tokio::test]
async fn persona_proposal_uses_native_receipts_without_creating_product_records() {
    let proposed = persona::starter("french").unwrap();
    let body = serde_json::to_string(&proposed).unwrap();
    let (url, worker) = crate::application::test_server::structured_server(move |_| body);
    let (directory, app) = generation_app();
    app.lock()
        .unwrap()
        .connection
        .execute(
            "UPDATE ai_config SET custom_config=json_set(custom_config,'$.baseUrl',?1)",
            [url],
        )
        .unwrap();
    app.lock().unwrap().connection.execute(
        "UPDATE learner SET preferences=json_set(preferences,'$.explanationLanguage','spanish','$.explanationVarietyId','spanish-spain')", []
    ).unwrap();
    let id =
        reserve_persona_generation(&app, "french".into(), Some("PRIVATE-BRIEF".into())).unwrap();
    let details = run_owned_persona_generation(&app, &id).await.unwrap();
    crate::ai::inspection::verify_workspace_reads(&app.lock().unwrap());
    assert_eq!(details.name, proposed.name);
    let payload = worker.join().unwrap();
    let request = &payload["items"][0]["request"];
    let prompt = request["messages"][0]["content"].as_str().unwrap();
    assert!(prompt.contains("Explanatory language: spanish (spanish-spain)."));
    assert!(prompt.contains("Use Spanish"));
    assert!(prompt.contains("Target language: French."));
    assert_eq!(
        request["response_format"]["json_schema"]["schema"]["properties"]["romanizedName"]["type"],
        "null"
    );
    assert!(run_owned_persona_generation(&app, &id).await.is_err());
    drop(app);
    let store = Store::open(&directory.path().join("generation.sqlite3")).unwrap();
    assert_eq!(
        store
            .connection
            .query_row(
                "SELECT count(*) FROM sqlite_master WHERE name='inference_executions'",
                [],
                |r| r.get::<_, i64>(0)
            )
            .unwrap(),
        0
    );
    assert_eq!(
        store
            .connection
            .query_row(
                "SELECT count(*) FROM workspace_graph_transport_identities",
                [],
                |r| r.get::<_, i64>(0)
            )
            .unwrap(),
        1
    );
    let snapshot = store.snapshot().unwrap();
    assert!(snapshot.personas.is_empty());
    assert!(snapshot.contacts.is_empty());
    assert!(snapshot.conversations.is_empty());
    let activity = generation_receipts::activity(&store.connection).unwrap();
    assert_eq!(activity.attempts[0].state, "succeeded");
    assert_eq!(activity.usage.attempts, 1);
    assert_eq!(activity.usage.input_tokens, 21);
    let source = &activity.attempts[0].diagnostics.as_ref().unwrap()["sourceExecution"];
    assert_eq!(source["state"], "succeeded");
    assert!(!source.to_string().contains(&proposed.name));
    assert!(!source.to_string().contains("PRIVATE-BRIEF"));
    assert_eq!(
        crate::ai::results::settings(&store.connection)
            .unwrap()
            .result_count,
        0
    );
    let profile = store.profile().unwrap();
    assert_eq!(profile.global.attempts, 1);
    assert_eq!(profile.global.input_tokens, 21);
    assert_eq!(
        profile
            .languages
            .iter()
            .find(|l| l.id == "french")
            .unwrap()
            .attempts,
        1
    );
}

fn generation_app() -> (tempfile::TempDir, Arc<Application>) {
    let directory = tempfile::tempdir().unwrap();
    let app = Application::start(&directory.path().join("generation.sqlite3"), None);
    app.lock().unwrap().connection.execute("UPDATE ai_config SET route='custom',custom_config=json_set(custom_config,'$.baseUrl','http://127.0.0.1:8765/v1','$.bearerAuth',json('false'))", []).unwrap();
    (directory, app)
}

#[test]
fn failed_durable_begin_releases_volatile_ownership_and_cancel_is_idempotent() {
    let (_directory, app) = generation_app();
    app.lock()
        .unwrap()
        .connection
        .execute_batch("PRAGMA query_only=ON")
        .unwrap();
    for _ in 0..5 {
        let error = reserve_persona_generation(&app, "spanish".into(), None).unwrap_err();
        assert_eq!(error.code, ErrorCode::Storage);
    }
    app.lock()
        .unwrap()
        .connection
        .execute_batch("PRAGMA query_only=OFF")
        .unwrap();
    let ids: Vec<_> = (0..4)
        .map(|_| reserve_persona_generation(&app, "spanish".into(), None).unwrap())
        .collect();
    for id in ids {
        cancel_owned_persona_generation(&app, &id).unwrap();
        cancel_owned_persona_generation(&app, &id).unwrap();
        assert!(app.generations.claim(&id).is_err());
    }
    assert!(reserve_persona_generation(&app, "spanish".into(), None).is_ok());
}

#[test]
fn cancellation_or_authority_change_cannot_adopt_a_completed_proposal() {
    for cancel in [true, false] {
        let (_directory, app) = generation_app();
        let id = reserve_persona_generation(&app, "spanish".into(), None).unwrap();
        let run = {
            let mut store = app.lock().unwrap();
            let run = app.generations.claim(&id).unwrap();
            generation_receipts::dispatch(&mut store, &run.request).unwrap();
            run.request.mark_submitted();
            run
        };
        if cancel {
            cancel_owned_persona_generation(&app, &id).unwrap();
        } else {
            app.lock()
                .unwrap()
                .connection
                .execute("UPDATE ai_config SET revision=revision+1", [])
                .unwrap();
        }
        let proposed = persona::starter("spanish").unwrap();
        let completion = provider::Completion {
            diagnostics: None,
            text: serde_json::to_string(&proposed).unwrap(),
            actual_model: "fixture".into(),
            provider_id: "synthetic".into(),
            finish_reason: "stop".into(),
            input_tokens: Some(4),
            output_tokens: Some(8),
        };
        let error = finish_persona_generation(&app, &run.request, Some(&completion), Ok(proposed))
            .unwrap_err();
        assert_eq!(error.code, ErrorCode::UnknownOutcome);
    }
}

#[test]
fn rejected_proposals_keep_usage_metadata_and_failed_terminal_writes_do_not_adopt() {
    for reject_write in [true, false] {
        let (_directory, app) = generation_app();
        let id = reserve_persona_generation(&app, "spanish".into(), Some("private brief".into()))
            .unwrap();
        let run = {
            let mut store = app.lock().unwrap();
            let run = app.generations.claim(&id).unwrap();
            generation_receipts::dispatch(&mut store, &run.request).unwrap();
            run.request.mark_submitted();
            run
        };
        let completion = provider::Completion {
            diagnostics: None,
            text: "private malformed proposal".into(),
            actual_model: "fixture-actual".into(),
            provider_id: "synthetic".into(),
            finish_reason: "stop".into(),
            input_tokens: Some(4),
            output_tokens: Some(8),
        };
        let outcome = if reject_write {
            app.lock()
                .unwrap()
                .connection
                .execute_batch("PRAGMA query_only=ON")
                .unwrap();
            Ok(persona::starter("spanish").unwrap())
        } else {
            generated_persona(&completion.text, "spanish")
        };
        let error =
            finish_persona_generation(&app, &run.request, Some(&completion), outcome).unwrap_err();
        if reject_write {
            assert_eq!(error.code, ErrorCode::Storage);
        } else {
            let activity = generation_receipts::activity(&app.lock().unwrap().connection).unwrap();
            assert_eq!(activity.attempts[0].state, "failed");
            assert_eq!(activity.attempts[0].input_tokens, Some(4));
            assert_eq!(activity.attempts[0].output_tokens, Some(8));
            assert_eq!(
                activity.attempts[0].actual_model.as_deref(),
                Some("fixture-actual")
            );
            assert!(
                !serde_json::to_string(&activity)
                    .unwrap()
                    .contains("private")
            );
        }
    }
}

#[test]
fn valid_proposal_publishes_independent_of_finish_label_with_usage() {
    for finish in ["length", "content_filter", "tool_calls", ""] {
        let (_directory, app) = generation_app();
        let id = reserve_persona_generation(&app, "spanish".into(), None).unwrap();
        let run = {
            let mut store = app.lock().unwrap();
            let run = app.generations.claim(&id).unwrap();
            generation_receipts::dispatch(&mut store, &run.request).unwrap();
            run.request.mark_submitted();
            run
        };
        let proposed = persona::starter("spanish").unwrap();
        let completed = Ok(provider::Completion {
            diagnostics: None,
            text: serde_json::to_string(&proposed).unwrap(),
            actual_model: "fixture".into(),
            provider_id: "synthetic".into(),
            finish_reason: finish.into(),
            input_tokens: Some(9),
            output_tokens: Some(14),
        });
        let outcome =
            generation::accept_completion(&mut app.lock().unwrap(), &run.request, &completed)
                .map(|_| proposed);
        finish_persona_generation(&app, &run.request, completed.as_ref().ok(), outcome).unwrap();
        let view = generation_receipts::activity(&app.lock().unwrap().connection).unwrap();
        assert_eq!(view.attempts[0].state, "succeeded");
        assert_eq!(view.attempts[0].input_tokens, Some(9));
        assert_eq!(view.attempts[0].output_tokens, Some(14));
        assert_eq!(view.usage.unknown_usage, 0);
    }
}

#[test]
fn generation_identities_have_the_shape_the_hosted_server_accepts() {
    let hex = |value: &str| {
        value.len() == 32
            && value
                .chars()
                .all(|c| c.is_ascii_digit() || ('a'..='f').contains(&c))
    };
    let (attempt, operation) = crate::ai::identity::new_execution_ids();
    // The server's rules: operation `[0-9a-f]{32}`, attempt `[0-9]{10}-[0-9a-f]{32}`.
    let (issued, random) = attempt.split_once('-').expect("attempt has an issue time");
    assert!(
        issued.len() == 10 && issued.chars().all(|c| c.is_ascii_digit()),
        "{attempt}"
    );
    assert!(hex(random), "{attempt}");
    assert!(hex(&operation), "{operation}");
}

#[test]
fn a_valid_response_becomes_a_reviewable_persona_and_writes_nothing() {
    let directory = tempfile::tempdir().unwrap();
    let store = Store::open(&directory.path().join("skellyspeak.sqlite3")).unwrap();
    let before = store.snapshot().unwrap();
    let proposed = persona::starter("french").unwrap();
    let details = generated_persona(&serde_json::to_string(&proposed).unwrap(), "french").unwrap();
    assert_eq!(details.name, proposed.name);
    assert_eq!(details.vibe, proposed.vibe);
    // A proposal is not a contact: nothing is written until the learner creates one.
    let after = store.snapshot().unwrap();
    assert_eq!(after.revision, before.revision);
    assert!(after.personas.is_empty());
    assert!(after.contacts.is_empty());
    assert!(after.conversations.is_empty());
}

#[test]
fn an_unusable_response_is_refused_and_writes_nothing() {
    let directory = tempfile::tempdir().unwrap();
    let store = Store::open(&directory.path().join("skellyspeak.sqlite3")).unwrap();
    let before = store.snapshot().unwrap().revision;
    assert_eq!(
        generated_persona("not json", "french").unwrap_err().code,
        ErrorCode::Provider
    );
    // Well-shaped but outside a limit: refused by the same rules an edit obeys.
    let mut oversized = persona::starter("french").unwrap();
    oversized.vibe = vec!["🌿".into()];
    assert_eq!(
        generated_persona(&serde_json::to_string(&oversized).unwrap(), "french")
            .unwrap_err()
            .code,
        ErrorCode::Validation
    );
    let after = store.snapshot().unwrap();
    assert_eq!(after.revision, before);
    assert!(after.personas.is_empty());
    assert!(after.contacts.is_empty());
    assert!(after.conversations.is_empty());
}

#[tokio::test]
async fn identical_proposals_are_fresh_native_runs() {
    let proposed = persona::starter("french").unwrap();
    let (url, worker) = crate::application::test_server::structured_sequence(2, move |_| {
        serde_json::to_string(&proposed).unwrap()
    });
    let (_directory, app) = generation_app();
    app.lock()
        .unwrap()
        .connection
        .execute(
            "UPDATE ai_config SET custom_config=json_set(custom_config,'$.baseUrl',?1)",
            [url],
        )
        .unwrap();
    for _ in 0..2 {
        let id = reserve_persona_generation(&app, "french".into(), None).unwrap();
        run_owned_persona_generation(&app, &id).await.unwrap();
    }
    let requests = worker.join().unwrap();
    assert_ne!(
        requests[0]["items"][0]["attempt_id"],
        requests[1]["items"][0]["attempt_id"]
    );
    let store = app.lock().unwrap();
    assert_eq!(
        store
            .connection
            .query_row(
                "SELECT count(*) FROM sqlite_master WHERE name='inference_executions'",
                [],
                |r| r.get::<_, i64>(0)
            )
            .unwrap(),
        0
    );
    assert_eq!(
        store
            .connection
            .query_row(
                "SELECT count(*) FROM workspace_graph_transport_identities",
                [],
                |r| r.get::<_, i64>(0)
            )
            .unwrap(),
        2
    );
    assert_eq!(store.profile().unwrap().global.attempts, 2);
    assert!(store.snapshot().unwrap().personas.is_empty());
}

#[tokio::test]
async fn malformed_native_persona_response_keeps_validation_error_and_provider_evidence() {
    let (url, worker) = crate::application::test_server::structured_server(|_| "{".into());
    let (_directory, app) = generation_app();
    app.lock()
        .unwrap()
        .connection
        .execute(
            "UPDATE ai_config SET custom_config=json_set(custom_config,'$.baseUrl',?1)",
            [url],
        )
        .unwrap();
    let id = reserve_persona_generation(&app, "french".into(), None).unwrap();
    let error = run_owned_persona_generation(&app, &id).await.unwrap_err();
    assert_eq!(error.code, ErrorCode::Validation);
    worker.join().unwrap();
    let store = app.lock().unwrap();
    let activity = generation_receipts::activity(&store.connection).unwrap();
    assert_eq!(activity.attempts[0].state, "failed");
    assert_eq!(
        activity.attempts[0].diagnostics.as_ref().unwrap()["sourceExecution"]["response"]["providerId"],
        "structured-receipt"
    );
    assert_eq!(activity.usage.attempts, 1);
    assert!(store.snapshot().unwrap().personas.is_empty());
}
