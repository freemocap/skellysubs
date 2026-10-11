use super::*;
mod performance;
fn fixture() -> (tempfile::TempDir, Store) {
    let dir = tempfile::tempdir().unwrap();
    let store = Store::open(&dir.path().join("reading.sqlite3")).unwrap();
    store.connection.execute("UPDATE ai_config SET route='custom',custom_config=json_set(custom_config,'$.baseUrl','http://127.0.0.1:8765/v1','$.bearerAuth',json('false'))", []).unwrap();
    (dir, store)
}
fn input() -> ReadingInput {
    ReadingInput {
        conversation_id: None,
        reference_item: None,
        text: "Hola".into(),
        language: "spanish".into(),
        variety: None,
        explanation: "english".into(),
        explanation_variety: None,
        aid: ReadingAid::WordGloss,
    }
}

#[test]
fn exploration_requires_new_successful_text_and_survives_cache_regeneration() {
    use crate::learning::effort::{exploration, read};
    let (_dir, store) = fixture();
    let request = Request::capture(&store, input()).unwrap();
    for receipt in [
        serde_json::json!({"state":"pending","response":{"cacheHit":false}}),
        serde_json::json!({"state":"failed","response":{"cacheHit":false}}),
        serde_json::json!({"state":"cancelled","response":{"cacheHit":false}}),
        serde_json::json!({"state":"succeeded","response":{"cacheHit":true}}),
        serde_json::json!({"state":"succeeded","response":{}}),
    ] {
        exploration::reading(&store.connection, &request, &receipt).unwrap();
    }
    assert_eq!(read(&store.connection, "spanish").unwrap().explorations, 0);
    let accepted = serde_json::json!({"state":"succeeded","response":{"cacheHit":false}});
    let transaction = store.connection.unchecked_transaction().unwrap();
    exploration::reading(&transaction, &request, &accepted).unwrap();
    transaction.rollback().unwrap();
    assert_eq!(read(&store.connection, "spanish").unwrap().explorations, 0);
    exploration::reading(&store.connection, &request, &accepted).unwrap();
    let mut retry = Request::capture(&store, input()).unwrap();
    retry.fresh = true;
    exploration::reading(&store.connection, &retry, &accepted).unwrap();
    assert_eq!(read(&store.connection, "spanish").unwrap().explorations, 1);
    let mut other = input();
    other.aid = ReadingAid::Translation;
    exploration::reading(
        &store.connection,
        &Request::capture(&store, other).unwrap(),
        &accepted,
    )
    .unwrap();
    assert_eq!(read(&store.connection, "spanish").unwrap().explorations, 2);
    let mut speech = input();
    speech.aid = ReadingAid::Speech;
    exploration::reading(
        &store.connection,
        &Request::capture(&store, speech).unwrap(),
        &accepted,
    )
    .unwrap();
    let progress = read(&store.connection, "spanish").unwrap();
    assert_eq!(progress.explorations, 2);
    assert!(
        progress
            .recent
            .iter()
            .all(|award| award.conversation_id.is_none())
    );
    assert_eq!(read(&store.connection, "french").unwrap().explorations, 0);
}

#[test]
fn exploration_rejects_missing_or_wrong_language_conversation_attribution() {
    let (_dir, mut store) = fixture();
    let execute = |store: &mut Store, action| {
        store
            .execute(Command {
                session_id: store.session_id.clone(),
                action_id: uuid::Uuid::new_v4().to_string(),
                action,
            })
            .unwrap()
            .entity_id
    };
    execute(
        &mut store,
        Action::CreateContact {
            language_id: "spanish".into(),
            details: crate::partners::persona::starter("spanish").unwrap(),
        },
    );
    let contact = store.snapshot().unwrap().contacts[0].id.clone();
    let conversation = execute(
        &mut store,
        Action::CreateConversation {
            contact_id: contact,
            title: "Reading".into(),
        },
    );
    let mut scoped = input();
    scoped.conversation_id = Some(conversation.clone());
    let request = Request::capture(&store, scoped.clone()).unwrap();
    crate::learning::effort::exploration::reading(
        &store.connection,
        &request,
        &serde_json::json!({"state":"succeeded","response":{"cacheHit":false}}),
    )
    .unwrap();
    let effort = crate::learning::effort::read(&store.connection, "spanish").unwrap();
    assert_eq!(effort.explorations, 1);
    assert_eq!(
        effort.recent[0].conversation_id.as_deref(),
        Some(conversation.as_str())
    );
    scoped.language = "french".into();
    assert!(Request::capture(&store, scoped.clone()).is_err());
    scoped.language = "spanish".into();
    scoped.conversation_id = Some("missing".into());
    assert!(Request::capture(&store, scoped).is_err());
    store
        .connection
        .execute("DELETE FROM conversations WHERE id=?1", [&conversation])
        .unwrap();
    assert!(request.validate_source(&store).is_err());
    assert_eq!(
        crate::learning::effort::read(&store.connection, "spanish")
            .unwrap()
            .explorations,
        1
    );
}
#[test]
fn requests_are_bounded_single_use_and_source_owned() {
    let (_dir, store) = fixture();
    let registry = Registry::default();
    let mut invalid = input();
    invalid.text = "x".repeat(2049);
    assert!(registry.begin(&store, invalid).is_err());
    let id = registry.begin(&store, input()).unwrap();
    let request = registry.claim(&id).unwrap();
    assert_eq!(request.source().text, "Hola");
    assert!(registry.claim(&id).is_err());
    registry.cancel(&store, &id).unwrap();
    assert_eq!(
        request.validate_source(&store).unwrap_err().code,
        ErrorCode::Conflict
    );
    let receipt = finish(
        &store,
        &request,
        serde_json::json!({"providerId":"receipt-123","inputTokens":8}),
        Some(&request.stopped("cancelled")),
    )
    .unwrap();
    assert_eq!(receipt["state"], "cancelled");
    assert_eq!(receipt["response"]["providerId"], "receipt-123");
    assert_eq!(receipt["error"]["diagnostics"]["reason"], "cancelled");
    assert!(receipt["error"]["diagnostics"]["dispatched"].is_null());
    assert!(!receipt.to_string().contains("Hola"));
}
#[test]
fn connection_change_and_pause_revoke_reading_without_touching_messages() {
    let (_dir, store) = fixture();
    let request = Request::capture(&store, input()).unwrap();
    store
        .connection
        .execute("UPDATE ai_config SET revision=revision+1", [])
        .unwrap();
    let changed = request.validate_source(&store).unwrap_err();
    assert_eq!(
        changed.diagnostics.as_ref().unwrap()["reason"],
        "access_changed"
    );
    assert!(changed.diagnostics.as_ref().unwrap()["dispatched"].is_null());
    store
        .connection
        .execute("UPDATE ai_config SET paused=1", [])
        .unwrap();
    let paused = Request::capture(&store, input())
        .unwrap()
        .validate_execution(&store)
        .unwrap_err();
    assert_eq!(paused.diagnostics.as_ref().unwrap()["reason"], "paused");
}
#[test]
fn recovery_cancels_unsubmitted_reading_requests() {
    let (_dir, store) = fixture();
    let registry = Registry::default();
    let pending = registry.begin(&store, input()).unwrap();
    let active = registry.begin(&store, input()).unwrap();
    registry.claim(&active).unwrap();
    recover(&store.connection).unwrap();
    let receipts = activity(&store).unwrap();
    assert!(
        receipts
            .iter()
            .any(|r| r["id"] == pending && r["state"] == "cancelled")
    );
    assert!(
        receipts
            .iter()
            .any(|r| r["id"] == active && r["state"] == "cancelled")
    );
}

#[test]
fn reading_speech_uses_language_capability_and_keeps_canonical_tag() {
    let (_dir, store) = fixture();
    let mut source = input();
    source.aid = ReadingAid::Speech;
    source.language = "irish".into();
    source.text = "Go raibh maith agat".into();
    let request = Request::capture(&store, source.clone()).unwrap();
    assert_eq!(request.target.model, "eleven_v3");
    assert_eq!(
        request.target.audio_resolution.as_ref().unwrap().provider,
        "elevenlabs"
    );
    assert_eq!(request.speech_input().unwrap().language_tag, "ga");
    request.validate_execution(&store).unwrap();
    source.language = "unsupported-language".into();
    assert!(Request::capture(&store, source).is_err());
}

#[test]
fn template_analysis_requests_completions_with_original_context() {
    let (_dir, store) = fixture();
    let mut source = input();
    source.aid = ReadingAid::Explanations;
    source.text = "Quiero __.".into();
    let request = Request::capture(&store, source).unwrap();
    let (dispatch, schema) = explanation_graph::decode(&request.native_inputs().unwrap())
        .unwrap()
        .prepare(request.attempt.clone(), request.operation.clone())
        .unwrap();
    assert_eq!(schema["properties"]["cards"]["minItems"], 2);
    assert_eq!(schema["properties"]["cards"]["maxItems"], 3);
    assert!(
        dispatch.messages[0]
            .content
            .contains(sentence_blanks::INSTRUCTION)
    );
    assert!(dispatch.messages[1].content.contains("Quiero __."));
}

#[test]
fn explicit_completion_aid_uses_template_syntax_and_requires_a_slot() {
    let (_dir, store) = fixture();
    let mut source = input();
    source.aid = ReadingAid::Completions;
    assert!(Request::capture(&store, source.clone()).is_err());
    source.text = "Quiero___hoy.".into();
    let request = Request::capture(&store, source).unwrap();
    let (dispatch, schema) = explanation_graph::decode(&request.native_inputs().unwrap())
        .unwrap()
        .prepare(request.attempt.clone(), request.operation.clone())
        .unwrap();
    assert_eq!(request.input.aid.receipt_kind(), "reading_completions");
    assert_eq!(schema["properties"]["cards"]["minItems"], 2);
    assert_eq!(schema["properties"]["cards"]["maxItems"], 3);
    assert!(
        dispatch.messages[0]
            .content
            .contains(sentence_blanks::INSTRUCTION)
    );
    assert!(dispatch.messages[1].content.contains("Quiero___hoy."));
}

#[test]
fn independent_reading_consumers_do_not_exhaust_local_request_slots() {
    let (_dir, store) = fixture();
    let registry = Registry::default();
    let mut requests = vec![];
    for _ in 0..16 {
        let id = registry.begin(&store, input()).unwrap();
        let request = registry.claim(&id).unwrap();
        request.validate_source(&store).unwrap();
        requests.push(request);
    }
    registry.cancel(&store, &requests[0].id).unwrap();
    assert!(requests[0].validate_source(&store).is_err());
    for request in &requests[1..] {
        request.validate_source(&store).unwrap();
    }
    for request in requests {
        registry.remove(&request.id).unwrap();
    }
    assert!(registry.0.lock().unwrap().is_empty());
}
