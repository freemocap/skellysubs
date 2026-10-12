use super::*;
use crate::application::test_server::{structured_server, translation_server};
use std::io::{Read, Write};

#[tokio::test]
async fn selected_word_reaches_speech_and_receipt_survives_without_source_content() {
    speech_request(false).await;
}

#[tokio::test]
async fn drill_reference_survives_restart_and_replay_does_not_dispatch_or_charge() {
    speech_request(true).await;
}

async fn speech_request(reference: bool) {
    let directory = tempfile::tempdir().unwrap();
    let state = Application::start(&directory.path().join("reading.sqlite3"), None);
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let base = format!("http://{}/v1", listener.local_addr().unwrap());
    let target = {
        let store = state.lock().unwrap();
        store.connection.execute("UPDATE ai_config SET route='custom',custom_config=json_set(custom_config,'$.baseUrl',?1,'$.bearerAuth',json('false'))", [&base]).unwrap();
        access::resolve(&store.connection, access::Capability::Speech).unwrap()
    };
    let body = serde_json::json!({"version":3,"response":{"audio_base64":"ZAA=","alignment":{"characters":["ك","ت","ا","ب"],"character_start_times_seconds":[0,0,0,0],"character_end_times_seconds":[0.00001,0.00001,0.00001,0.00001]}},"usage":{"requested_model":target.model,"actual_model":"eleven_v3","provider":"elevenlabs","request_id":"speech-receipt","cost_micros":null,"allowance_micros":12}}).to_string();
    let worker = std::thread::spawn(move || {
        let (mut socket, _) = listener.accept().unwrap();
        socket
            .set_read_timeout(Some(Duration::from_secs(5)))
            .unwrap();
        let mut request = Vec::new();
        loop {
            let mut buffer = [0; 4096];
            let count = socket.read(&mut buffer).unwrap();
            assert!(count > 0);
            request.extend_from_slice(&buffer[..count]);
            let text = String::from_utf8_lossy(&request);
            if let Some((headers, payload)) = text.split_once("\r\n\r\n") {
                let length: usize = headers
                    .lines()
                    .find_map(|line| {
                        line.to_lowercase()
                            .strip_prefix("content-length: ")
                            .and_then(|n| n.parse().ok())
                    })
                    .unwrap();
                if payload.len() >= length {
                    break;
                }
            }
        }
        let request = String::from_utf8(request).unwrap();
        assert!(request.starts_with("POST /v1/audio/speech"));
        let payload: serde_json::Value =
            serde_json::from_str(request.split_once("\r\n\r\n").unwrap().1).unwrap();
        assert_eq!(payload.as_object().unwrap().len(), 3);
        assert!(payload["language_code"].is_null());
        assert_eq!(payload["text"], "كتاب");
        write!(
            socket,
            "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
            body.len(),
            body
        )
        .unwrap();
    });
    let item = if reference {
        Some(
            state
                .lock()
                .unwrap()
                .create_drill_item(crate::drill::DrillItemInput {
                    text: "كتاب".into(),
                    language: "arabic".into(),
                    variety: None,
                    explanation: "english".into(),
                    explanation_variety: None,
                })
                .unwrap()
                .id,
        )
    } else {
        None
    };
    let id = state
        .reading
        .begin(
            &state.lock().unwrap(),
            reading::ReadingInput {
                conversation_id: None,
                reference_item: item.clone(),
                text: "كتاب".into(),
                language: "arabic".into(),
                variety: None,
                explanation: "english".into(),
                explanation_variety: None,
                aid: crate::language::reading::ReadingAid::Speech,
            },
        )
        .unwrap();
    let companion_input = reading::ReadingInput {
        conversation_id: None,
        reference_item: None,
        text: "\u{0643}\u{062a}\u{0627}\u{0628}".into(),
        language: "arabic".into(),
        variety: None,
        explanation: "english".into(),
        explanation_variety: None,
        aid: reading::ReadingAid::Speech,
    };
    let independent =
        reading::Request::capture(&state.lock().unwrap(), companion_input.clone()).unwrap();
    assert!(
        cached_reading_audio(&state.lock().unwrap(), companion_input.clone())
            .unwrap()
            .is_none()
    );
    assert_eq!(
        state
            .lock()
            .unwrap()
            .connection
            .query_row::<i64, _, _>(
                "SELECT count(*) FROM sqlite_master WHERE name='inference_executions'",
                [],
                |r| r.get(0)
            )
            .unwrap(),
        0
    );
    let companion = state
        .reading
        .begin(&state.lock().unwrap(), companion_input)
        .unwrap();
    let (result, other, third) = tokio::join!(
        run_owned_reading(&state, &id),
        run_owned_reading(&state, &companion),
        state.shared_speech(
            independent.target.clone(),
            independent.speech_input().unwrap(),
            independent.install.clone(),
            "independent-consumer",
            || Ok(())
        )
    );
    let result = result.unwrap();
    let other = other.unwrap();
    let third = third.unwrap();
    assert_eq!(result.audio_base64, other.audio_base64);
    assert_eq!(result.audio_alignment, other.audio_alignment);
    assert_eq!(
        result
            .audio_alignment
            .as_ref()
            .unwrap()
            .original
            .as_ref()
            .unwrap()
            .characters
            .len(),
        4
    );
    assert_eq!(
        result.receipt["response"]["sourceExecutionId"],
        other.receipt["response"]["sourceExecutionId"]
    );
    assert_eq!(
        third.execution,
        result.receipt["response"]["sourceExecutionId"]
            .as_str()
            .unwrap()
    );
    let paid: i64 = state
        .lock()
        .unwrap()
        .connection
        .query_row(
            "SELECT count(*) FROM workspace_graph_transport_identities",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(paid, 1);
    assert_eq!(
        state
            .lock()
            .unwrap()
            .connection
            .query_row(
                "SELECT count(*) FROM sqlite_master WHERE name='inference_executions'",
                [],
                |r| r.get::<_, i64>(0)
            )
            .unwrap(),
        0
    );
    let profile = state.lock().unwrap().profile().unwrap();
    assert_eq!(profile.global.attempts, 1);
    assert_eq!(
        profile
            .languages
            .iter()
            .find(|v| v.id == "arabic")
            .unwrap()
            .attempts,
        1
    );

    worker.join().unwrap();
    assert!(result.audio_base64.is_some());
    assert!(result.gloss.is_none());
    let receipts = reading::activity(&state.lock().unwrap()).unwrap();
    assert_eq!(receipts[0]["state"], "succeeded");
    assert_eq!(receipts[0]["response"]["providerId"], "speech-receipt");
    assert!(receipts[0]["response"]["costMicros"].is_null());
    // The receipt keeps the same content-free speech projection persona speech
    // retains. This route reports no transcript comparison, so the field is
    // explicitly null rather than dropped; the shared projection's own test
    // covers retention when a provider does send one.
    let response = &receipts[0]["response"];
    assert_eq!(response["audioAccepted"], true);
    assert!(response.get("finishReason").is_some());
    assert!(
        response
            .get("transcriptComparison")
            .is_some_and(serde_json::Value::is_null)
    );
    assert!(!receipts[0].to_string().contains("كتاب"));
    assert!(!receipts[0].to_string().contains("ZAA="));
    assert!(state.reading.claim(&id).is_err());
    {
        drop(state);
        let state = Application::start(&directory.path().join("reading.sqlite3"), None);
        state
            .lock()
            .unwrap()
            .connection
            .execute("UPDATE ai_config SET paused=1", [])
            .unwrap();
        let before: i64 = state.lock().unwrap().connection.query_row("SELECT count(*) FROM reading_attempts WHERE json_extract(receipt,'$.dispatchedAt') IS NOT NULL", [], |r| r.get(0)).unwrap();
        let input = reading::ReadingInput {
            conversation_id: None,
            reference_item: item,
            text: "كتاب".into(),
            language: "arabic".into(),
            variety: None,
            explanation: "english".into(),
            explanation_variety: None,
            aid: reading::ReadingAid::Speech,
        };
        let cached_id = state
            .reading
            .begin(&state.lock().unwrap(), input.clone())
            .unwrap();
        let records_before: i64 = state
            .lock()
            .unwrap()
            .connection
            .query_row("SELECT count(*) FROM reading_attempts", [], |r| r.get(0))
            .unwrap();
        assert_eq!(
            cached_reading_audio(&state.lock().unwrap(), input.clone())
                .unwrap()
                .unwrap()
                .alignment,
            result.audio_alignment
        );
        assert_eq!(
            cached_reading_audio(&state.lock().unwrap(), input.clone())
                .unwrap()
                .map(|audio| audio.audio_base64),
            result.audio_base64
        );
        assert_eq!(
            state
                .lock()
                .unwrap()
                .connection
                .query_row::<i64, _, _>("SELECT count(*) FROM reading_attempts", [], |r| r.get(0))
                .unwrap(),
            records_before
        );
        let duplicate = state.reading.begin(&state.lock().unwrap(), input).unwrap();
        // The only server has shut down: any second provider call fails this test.
        let cached = run_owned_reading(&state, &cached_id).await.unwrap();
        assert_eq!(cached.audio_base64, result.audio_base64);
        assert_eq!(cached.receipt["response"]["cacheHit"], true);
        assert_eq!(
            cached.receipt["response"]["sourceExecutionId"],
            result.receipt["response"]["sourceExecutionId"]
        );
        assert_eq!(
            run_owned_reading(&state, &duplicate)
                .await
                .unwrap()
                .audio_base64,
            result.audio_base64
        );
        assert!(cached.receipt.get("dispatchedAt").is_none());
        let after: i64 = state.lock().unwrap().connection.query_row("SELECT count(*) FROM reading_attempts WHERE json_extract(receipt,'$.dispatchedAt') IS NOT NULL", [], |r| r.get(0)).unwrap();
        assert_eq!(before, after);
        assert_eq!(state.lock().unwrap().profile().unwrap().global.attempts, 1);
    }
}

#[tokio::test]
async fn translation_runs_the_conversation_translation_contract_and_is_counted() {
    let directory = tempfile::tempdir().unwrap();
    let state = Application::start(&directory.path().join("reading.sqlite3"), None);
    let (base, worker) = translation_server(serde_json::json!("The book."));
    let (fast, expected_messages) = {
        let store = state.lock().unwrap();
        store.connection.execute("UPDATE ai_config SET route='custom',custom_config=json_set(custom_config,'$.baseUrl',?1,'$.bearerAuth',json('false'))", [&base]).unwrap();
        let context = store
            .config
            .resolve_pair("arabic", None, "english", None)
            .unwrap();
        (
            crate::ai::connections::configuration::config(&store.connection)
                .unwrap()
                .fast_model,
            crate::language::translation::Request {
                source: "كتاب".into(),
                source_language: "arabic".into(),
                destination_language: "english".into(),
                destination_writing: context.guidance("explanation_writing"),
            }
            .messages(),
        )
    };
    let before = state.lock().unwrap().profile().unwrap();
    let id = state
        .reading
        .begin(
            &state.lock().unwrap(),
            reading::ReadingInput {
                conversation_id: None,
                reference_item: None,
                text: "كتاب".into(),
                language: "arabic".into(),
                variety: None,
                explanation: "english".into(),
                explanation_variety: None,
                aid: reading::ReadingAid::Translation,
            },
        )
        .unwrap();
    let result = run_owned_reading(&state, &id).await.unwrap();
    let payload = worker.join().unwrap();
    assert_eq!(result.translation.as_deref(), Some("The book."));
    assert!(result.gloss.is_none() && result.audio_base64.is_none());
    // The request is the translation turn contract: prompt, schema, role, temperature.
    let request = &payload["items"][0]["request"];
    assert_eq!(
        request["messages"],
        serde_json::to_value(&expected_messages).unwrap()
    );
    assert_eq!(request["model"], fast);
    assert_eq!(
        request["temperature"],
        crate::ai::connections::model_routing::TASK_TEMPERATURE
    );
    assert_eq!(
        request["response_format"]["json_schema"]["schema"],
        crate::language::translation::schema()
    );
    let receipts = reading::activity(&state.lock().unwrap()).unwrap();
    assert_eq!(receipts[0]["kind"], "reading_translation");
    assert_eq!(receipts[0]["state"], "succeeded");
    assert_eq!(receipts[0]["requestedModel"], fast);
    assert_eq!(receipts[0]["response"]["providerId"], "structured-receipt");
    assert!(!receipts[0].to_string().contains("كتاب"));
    assert!(!receipts[0].to_string().contains("The book."));
    // Usage is counted globally and for the language, never for a partner.
    let after = state.lock().unwrap().profile().unwrap();
    assert_eq!(after.global.attempts, before.global.attempts + 1);
    assert_eq!(after.global.input_tokens, before.global.input_tokens + 21);
    assert_eq!(after.global.output_tokens, before.global.output_tokens + 4);
    let language = |profile: &model::ProfileSnapshot| {
        profile
            .languages
            .iter()
            .find(|l| l.id == "arabic")
            .map(|l| l.attempts)
    };
    assert_eq!(language(&after), language(&before).map(|n| n + 1));
    assert_eq!(
        after
            .personas
            .iter()
            .map(|p| p.attempts)
            .collect::<Vec<_>>(),
        before
            .personas
            .iter()
            .map(|p| p.attempts)
            .collect::<Vec<_>>()
    );
}

#[tokio::test]
async fn unclear_translation_fails_with_a_receipt_and_still_counts_usage() {
    let directory = tempfile::tempdir().unwrap();
    let state = Application::start(&directory.path().join("reading.sqlite3"), None);
    let (base, worker) = translation_server(serde_json::Value::Null);
    state.lock().unwrap().connection.execute("UPDATE ai_config SET route='custom',custom_config=json_set(custom_config,'$.baseUrl',?1,'$.bearerAuth',json('false'))", [&base]).unwrap();
    let before = state.lock().unwrap().profile().unwrap().global.attempts;
    let id = state
        .reading
        .begin(
            &state.lock().unwrap(),
            reading::ReadingInput {
                conversation_id: None,
                reference_item: None,
                text: "كتاب".into(),
                language: "arabic".into(),
                variety: None,
                explanation: "english".into(),
                explanation_variety: None,
                aid: reading::ReadingAid::Translation,
            },
        )
        .unwrap();
    let Err(error) = run_owned_reading(&state, &id).await else {
        panic!("an unclear translation must fail")
    };
    worker.join().unwrap();
    assert_eq!(
        error.message,
        "Translation: source meaning is unclear; no translation was published"
    );
    let receipts = reading::activity(&state.lock().unwrap()).unwrap();
    assert_eq!(receipts[0]["state"], "failed");
    assert_eq!(receipts[0]["response"]["providerId"], "structured-receipt");
    assert_eq!(
        state.lock().unwrap().profile().unwrap().global.attempts,
        before + 1
    );
}

#[tokio::test]
async fn word_gloss_runs_the_conversation_gloss_contract_and_keeps_partial_meanings() {
    let directory = tempfile::tempdir().unwrap();
    let state = Application::start(&directory.path().join("reading.sqlite3"), None);
    // No span is valid, so recovery keeps the whole source unresolved.
    let (base, worker) = structured_server(|_| r#"{"spans":[]}"#.into());
    let (model, expected) = {
        let store = state.lock().unwrap();
        store.connection.execute("UPDATE ai_config SET route='custom',custom_config=json_set(custom_config,'$.baseUrl',?1,'$.bearerAuth',json('false'))", [&base]).unwrap();
        let target = access::resolve(&store.connection, access::Capability::Chat).unwrap();
        let context = store
            .config
            .resolve_pair("arabic", None, "english", None)
            .unwrap();
        let identity = crate::language::linguistics::SourceIdentity {
            message_id: "unused".into(),
            target_language_id: "arabic".into(),
            explanation_language_id: "english".into(),
            analysis_version: crate::language::linguistics::ANALYSIS_VERSION.into(),
        };
        let prompt = crate::language::linguistics::adapter::build_word_gloss_prompt_with_context(
            &identity, "كتاب", &context,
        )
        .unwrap();
        (
            crate::ai::connections::model_routing::target(
                &target,
                crate::language::gloss::ROLE,
                &crate::ai::connections::configuration::config(&store.connection)
                    .unwrap()
                    .fast_model,
            )
            .model,
            prompt,
        )
    };
    let before = state.lock().unwrap().profile().unwrap();
    let id = state
        .reading
        .begin(
            &state.lock().unwrap(),
            reading::ReadingInput {
                conversation_id: None,
                reference_item: None,
                text: "كتاب".into(),
                language: "arabic".into(),
                variety: None,
                explanation: "english".into(),
                explanation_variety: None,
                aid: reading::ReadingAid::WordGloss,
            },
        )
        .unwrap();
    let result = run_owned_reading(&state, &id).await.unwrap();
    let payload = worker.join().unwrap();
    let gloss = result.gloss.unwrap();
    assert_eq!(gloss.coverage, model::GlossCoverage::Partial);
    assert!(result.translation.is_none() && result.audio_base64.is_none());
    let request = &payload["items"][0]["request"];
    assert_eq!(
        request["messages"],
        serde_json::to_value(&expected.messages).unwrap()
    );
    assert_eq!(request["model"], model);
    assert_eq!(
        request["temperature"],
        crate::ai::connections::model_routing::TASK_TEMPERATURE
    );
    assert_eq!(
        request["response_format"]["json_schema"]["schema"],
        expected.output_schema
    );
    let receipts = reading::activity(&state.lock().unwrap()).unwrap();
    assert_eq!(receipts[0]["kind"], "reading_gloss");
    assert_eq!(receipts[0]["state"], "succeeded");
    assert_eq!(receipts[0]["requestedModel"], model);
    assert_eq!(
        receipts[0]["response"]["wordGlossValidation"]["policy"],
        "word-gloss-recovery-v1"
    );
    assert!(!receipts[0].to_string().contains("كتاب"));
    let after = state.lock().unwrap().profile().unwrap();
    assert_eq!(after.global.attempts, before.global.attempts + 1);
}

#[tokio::test]
async fn explanations_run_the_conversation_support_contract_against_the_source() {
    let directory = tempfile::tempdir().unwrap();
    let state = Application::start(&directory.path().join("reading.sqlite3"), None);
    let (base, worker) = structured_server(|_| {
        serde_json::json!({"cards":[{"quote":"كتاب","title":"Noun","body":"A book.","example":"هذا كتاب","contrast":""}]}).to_string()
    });
    state.lock().unwrap().connection.execute("UPDATE ai_config SET route='custom',custom_config=json_set(custom_config,'$.baseUrl',?1,'$.bearerAuth',json('false'))", [&base]).unwrap();
    let before = state.lock().unwrap().profile().unwrap();
    let id = state
        .reading
        .begin(
            &state.lock().unwrap(),
            reading::ReadingInput {
                conversation_id: None,
                reference_item: None,
                text: "كتاب جديد".into(),
                language: "arabic".into(),
                variety: None,
                explanation: "english".into(),
                explanation_variety: None,
                aid: reading::ReadingAid::Explanations,
            },
        )
        .unwrap();
    let result = run_owned_reading(&state, &id).await.unwrap();
    let payload = worker.join().unwrap();
    let cards = result.explanations.unwrap().cards;
    assert_eq!(cards.len(), 1);
    assert_eq!(cards[0].quote, "كتاب");
    let request = &payload["items"][0]["request"];
    assert_eq!(
        request["temperature"],
        crate::ai::connections::model_routing::TASK_TEMPERATURE
    );
    assert_eq!(
        request["response_format"]["json_schema"]["schema"],
        crate::learning::coaching::conversation_support::schema("reply_explanations")
    );
    let receipts = reading::activity(&state.lock().unwrap()).unwrap();
    assert_eq!(receipts[0]["kind"], "reading_explanations");
    assert_eq!(receipts[0]["state"], "succeeded");
    assert!(!receipts[0].to_string().contains("كتاب"));
    let after = state.lock().unwrap().profile().unwrap();
    assert_eq!(after.global.attempts, before.global.attempts + 1);
}
