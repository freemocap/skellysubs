//! Host admission, claims, provider preparation and publication using an actual
//! command admission. The missing-criteria case injects a captured-content fault.
use super::*;
mod requested;
use crate::{
    ai::graph::*,
    language::{gloss_graph, translation_graph},
    learning::coaching::{assessment_graph, feedback_graph, support_graph},
};
use serde_json::json;
use std::sync::Arc;

#[tokio::test]
async fn native_partner_host_keeps_reply_and_helpers_running_after_assessment_failure() {
    exercise(false, false).await;
    exercise(true, false).await;
    exercise(false, true).await;
}

async fn exercise(speech: bool, missing_criteria: bool) {
    let (_dir, mut store, conversation) = setup();
    if speech {
        store.connection.execute("UPDATE conversation_settings SET settings=json_set(settings,'$.readAloud',json('true')) WHERE conversation_id=?1",[&conversation]).unwrap();
    }
    let turn = if missing_criteria {
        // Deliberately damaged captured material is a component fault fixture.
        let snapshot = store.snapshot().unwrap();
        let tx = store.connection.transaction().unwrap();
        let turn = capture_native_send(
            &tx,
            &store.config,
            &snapshot,
            &conversation,
            "Hola, ¿cómo estás?",
            snapshot.conversations[0].revision,
        )
        .unwrap();
        tx.execute(
            "UPDATE turns SET context=json_set(context,'$.input',json(?2)) WHERE id=?1",
            params![
                turn,
                serde_json::to_string(&crate::learning::coaching::InputEvidence::default())
                    .unwrap()
            ],
        )
        .unwrap();
        tx.execute("UPDATE turns SET context=json_set(context,'$.presenceSkills',NULL,'$.presenceContentError',json(?2)) WHERE id=?1",params![turn,serde_json::json!({"code":"validation","message":"Captured skill criteria unavailable"}).to_string()]).unwrap();
        store
            .graph_runtime
            .admit(
                tx,
                &turn,
                graph_runtime::Admission::Partner(context::Kind::Reply),
            )
            .unwrap();
        turn
    } else {
        let command = send(&store, &conversation);
        let receipt = store.execute(command.clone()).unwrap();
        assert_eq!(store.execute(command).unwrap().entity_id, receipt.entity_id);
        receipt.entity_id
    };
    assert_eq!(
        store
            .connection
            .query_row(
                "SELECT count(*) FROM operations WHERE turn_id=?1",
                [&turn],
                |r| r.get::<_, i64>(0)
            )
            .unwrap(),
        0
    );
    assert_eq!(
        store
            .connection
            .query_row(
                "SELECT executor FROM turn_execution_owners WHERE turn_id=?1",
                [&turn],
                |r| r.get::<_, String>(0)
            )
            .unwrap(),
        "graph"
    );
    let (sender, mut receiver) = tokio::sync::mpsc::unbounded_channel();
    store.graph_runtime.partner.bind(
        Arc::new(move |context| {
            let (answer, received) = tokio::sync::oneshot::channel();
            sender.send((context, answer)).unwrap();
            Box::pin(async move { received.await.unwrap() })
        }),
        Arc::new(|_, _| Box::pin(async { Ok(None) })),
    );
    let (speech_sender, mut speech_receiver) = tokio::sync::mpsc::unbounded_channel();
    let (lookup_sender, mut lookup_receiver) = tokio::sync::mpsc::unbounded_channel();
    store.graph_runtime.partner.bind_speech(
        Arc::new(move |context, _| {
            let (answer, received) = tokio::sync::oneshot::channel();
            speech_sender.send((context, answer)).unwrap();
            Box::pin(async move { received.await.unwrap() })
        }),
        Arc::new(move |context, _| {
            let (answer, received) = tokio::sync::oneshot::channel();
            lookup_sender.send((context, answer)).unwrap();
            Box::pin(async move { received.await.unwrap() })
        }),
    );
    for _ in 0..60 {
        let claim = store
            .graph_runtime
            .next(
                &mut store.connection,
                true,
                &store.config,
                &store.session_id,
            )
            .unwrap();
        let Some(claim) = claim else {
            continue;
        };
        let resource = claim.resource;
        let mut task = tokio::spawn(claim.invocation.execute(EvidenceLimits {
            observations: 16,
            bytes: 65536,
        }));
        let report = tokio::select! {
            report = &mut task => report.unwrap(),
            Some((invocation, answer)) = speech_receiver.recv() => {
                assert_eq!(resource, Resource::Provider);
                let request = store.graph_runtime.authorize_speech(&store.connection, invocation.identity()).unwrap();
                assert_eq!(request.source.text, "Hola.");
                let mut cursor = std::io::Cursor::new(Vec::new());
                {
                    let mut wav = hound::WavWriter::new(&mut cursor, hound::WavSpec { channels:1, sample_rate:24000, bits_per_sample:16, sample_format:hound::SampleFormat::Int }).unwrap();
                    wav.write_sample(42i16).unwrap();
                    wav.finalize().unwrap();
                }
                let receipt = store.graph_runtime.stage_audio(&store.connection, invocation.identity(), cursor.into_inner(), None).unwrap();
                assert_eq!(store.connection.query_row("SELECT count(*) FROM graph_audio_receipts",[],|r|r.get::<_,i64>(0)).unwrap(),0);
                answer.send(Ok(receipt)).unwrap();
                task.await.unwrap()
            }
            Some((invocation, answer)) = lookup_receiver.recv() => {
                assert_eq!(resource, Resource::Local);
                let cached = store.graph_runtime.lookup_audio(&store.connection, invocation.identity()).unwrap();
                assert!(cached.is_none());
                answer.send(Ok(cached)).unwrap();
                task.await.unwrap()
            }
            Some((invocation, answer)) = receiver.recv() => {
            let prepared = store
                .graph_runtime
                .authorize_text(&store.connection, invocation.identity())
                .unwrap();
            assert!(!prepared.request.attempt.is_empty());
            let operation = &invocation.identity().operation;
            let outcome = if *operation == assessment_graph::operation_contract() {
                assert!(prepared.request.decisions.is_some());
                Err(Fault {
                    code: "controlled_assessment_failure".into(),
                    path: "provider".into(),
                })
            } else {
                let value = if *operation == prose::operation_contract() {
                    assert!(matches!(
                        prepared.output(),
                        crate::ai::transport::provider::RequestOutput::Prose
                    ));
                    None
                } else {
                    assert!(matches!(
                        prepared.output(),
                        crate::ai::transport::provider::RequestOutput::JsonSchema { .. }
                    ));
                    Some(if *operation == translation_graph::operation_contract() {
                        json!({"source":prepared.request.messages[1].content,"translation":"Hello."})
                    } else if *operation == gloss_graph::operation_contract() {
                        json!({"spans":[]})
                    } else if *operation == feedback_graph::operation_contract() {
                        json!({"meaning_recovered":"full","items":[]})
                    } else if *operation == support_graph::Task::Brief.operation() {
                        json!({"explanation":"A greeting."})
                    } else {
                        panic!("unexpected provider operation: {operation:?}")
                    })
                };
                Ok(Completion {
                    text: value.map_or_else(|| "Hola.".into(), |v| v.to_string()),
                    finish_reason: "stop".into(),
                    actual_model: "fixture".into(),
                    provider_id: "fixture".into(),
                    input_tokens: Some(1),
                    output_tokens: Some(2),
                    diagnostics: None,
                })
            };
            answer.send(outcome).unwrap();
            task.await.unwrap()
            }
            _ = tokio::time::sleep(std::time::Duration::from_secs(10)) => panic!("Native fixture host did not complete or request a capability"),
        };
        assert!(
            report.outcome.is_ok()
                || report.identity.operation == assessment_graph::operation_contract(),
            "{:?}",
            report.outcome
        );
        if report.identity.operation == crate::speech::synthesis_graph::operation_contract() {
            store.connection.execute_batch("CREATE TEMP TRIGGER reject_native_audio BEFORE INSERT ON graph_audio_cache BEGIN SELECT RAISE(ABORT,'fixture audio retention failure'); END;").unwrap();
            assert!(
                store
                    .graph_runtime
                    .finish(&mut store.connection, &conversation, &turn, report.clone())
                    .is_err()
            );
            assert_eq!(
                store
                    .connection
                    .query_row("SELECT count(*) FROM graph_audio_receipts", [], |r| r
                        .get::<_, i64>(0))
                    .unwrap(),
                0
            );
            store
                .connection
                .execute_batch("DROP TRIGGER reject_native_audio;")
                .unwrap();
        }
        store
            .graph_runtime
            .finish(&mut store.connection, &conversation, &turn, report)
            .unwrap();
    }
    let snapshot = store
        .graph_runtime
        .inspection(&store.connection, &conversation, &turn)
        .unwrap()
        .unwrap();
    assert_eq!(snapshot.nodes["reply"], Disposition::Adopted);
    assert_eq!(snapshot.nodes["assessment"], Disposition::Failed);
    assert_eq!(snapshot.nodes["reply_translation"], Disposition::Adopted);
    assert_eq!(snapshot.nodes["learner_translation"], Disposition::Adopted);
    assert_eq!(snapshot.nodes["brief"], Disposition::Adopted);
    assert_eq!(
        snapshot.nodes["speech/lookup"],
        if speech {
            Disposition::Adopted
        } else {
            Disposition::Unrequested
        }
    );
    if speech {
        assert_eq!(snapshot.nodes["speech/audio"], Disposition::Adopted);
    }
    assert_eq!(
        store
            .connection
            .query_row("SELECT count(*) FROM graph_audio_receipts", [], |r| r
                .get::<_, i64>(0))
            .unwrap(),
        i64::from(speech)
    );
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
    let (state, raw): (String, String) = store
        .connection
        .query_row(
            "SELECT state,context FROM turns WHERE id=?1",
            [&turn],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .unwrap();
    assert_eq!(state, "succeeded");
    let context: serde_json::Value = serde_json::from_str(&raw).unwrap();
    assert_eq!(context["translation"], "Hello.");
    assert_eq!(context["userTranslation"], "Hello.");
    assert_eq!(
        store
            .connection
            .query_row(
                "SELECT count(*) FROM messages WHERE turn_id=?1 AND role='assistant'",
                [&turn],
                |r| r.get::<_, i64>(0)
            )
            .unwrap(),
        1
    );
    assert_eq!(
        store
            .connection
            .query_row(
                "SELECT count(*) FROM operations WHERE turn_id=?1",
                [&turn],
                |r| r.get::<_, i64>(0)
            )
            .unwrap(),
        0
    );
    let product = store.conversation_snapshot(&conversation, None).unwrap();
    assert_eq!(
        product
            .turns
            .iter()
            .filter_map(|t| t.speech.as_ref())
            .count(),
        usize::from(speech)
    );
    for message in &product.messages {
        assert_eq!(message.translation_state.as_deref(), Some("succeeded"));
        assert_eq!(message.gloss_state.as_deref(), Some("succeeded"));
        assert!(
            message
                .gloss_operation_id
                .as_ref()
                .unwrap()
                .starts_with("graph:")
        );
        if message.role == "assistant" {
            assert_eq!(message.brief_state.as_deref(), Some("succeeded"));
            assert_eq!(message.suggestions_state, None);
            assert_eq!(message.explanations_state, None);
        }
    }
    let assistant = product
        .messages
        .iter()
        .find(|m| m.role == "assistant")
        .unwrap()
        .id
        .clone();
    let learner = product
        .messages
        .iter()
        .find(|m| m.role == "user")
        .unwrap()
        .id
        .clone();
    for action in [
        Action::RequestSuggestions {
            message_id: assistant.clone(),
        },
        Action::RequestExplanations {
            message_id: assistant.clone(),
        },
        Action::RequestMessageHelp {
            message_id: learner,
            help: MessageHelp::Assessment,
            retry: true,
        },
    ] {
        assert!(apply(&mut store, action).entity_id.starts_with("graph:"));
    }
    let requested = store
        .graph_runtime
        .inspection(&store.connection, &conversation, &turn)
        .unwrap()
        .unwrap();
    assert_eq!(requested.nodes["assistance"], Disposition::Ready);
    assert_eq!(requested.nodes["explanation_evidence"], Disposition::Ready);
    assert_eq!(requested.nodes["assessment"], Disposition::Ready);
    assert_eq!(requested.nodes["reply"], Disposition::Adopted);
    let node = snapshot
        .artifact
        .definition
        .nodes
        .iter()
        .find(|(_, n)| n.operation == crate::speech::synthesis_graph::playback::select_operation())
        .unwrap()
        .0;
    let operation = format!(
        "graph:{}",
        serde_json::to_string(&(&snapshot.engine, &turn, node)).unwrap()
    );
    let state = store.speech_audio(&operation).unwrap();
    if speech {
        let SpeechAudioState::Ready {
            attempt_id,
            audio_base64,
            alignment,
            ..
        } = state
        else {
            panic!("Expected native playback, got {state:?}");
        };
        let audio = crate::speech::alignment::SpeechAudio {
            audio_base64,
            alignment,
        };
        assert!(store.speech_stream_execution(&operation).unwrap().is_some());
        assert!(
            store
                .delivered_speech_owner(&operation, &attempt_id, &audio)
                .is_ok()
        );
        let mut tampered = audio.clone();
        tampered.audio_base64.push('A');
        assert!(
            store
                .delivered_speech_owner(&operation, &attempt_id, &tampered)
                .is_err()
        );
        crate::ai::results::set_capacity(&store.connection, 0).unwrap();
        assert!(matches!(
            store.speech_audio(&operation).unwrap(),
            SpeechAudioState::Unavailable {
                reason: SpeechUnavailableReason::Expired,
                ..
            }
        ));
        assert!(
            store
                .delivered_speech_owner(&operation, &attempt_id, &audio)
                .is_ok()
        );
        drop(store);
        let mut reopened = Store::open(&_dir.path().join("test.sqlite3")).unwrap();
        assert!(matches!(
            reopened.speech_audio(&operation).unwrap(),
            SpeechAudioState::Unavailable {
                reason: SpeechUnavailableReason::Expired,
                ..
            }
        ));
        assert!(
            reopened
                .delivered_speech_owner(&operation, &attempt_id, &audio)
                .is_ok()
        );
        assert_eq!(
            reopened
                .connection
                .query_row("SELECT count(*) FROM graph_audio_receipts", [], |r| r
                    .get::<_, i64>(0))
                .unwrap(),
            1
        );
        requested::exercise(&mut reopened, &assistant).await;
        reopened
            .connection
            .execute("UPDATE turns SET state='invalidated' WHERE id=?1", [&turn])
            .unwrap();
        assert!(reopened.speech_audio(&operation).is_err());
        assert!(
            reopened
                .delivered_speech_owner(&operation, &attempt_id, &audio)
                .is_err()
        );
    } else {
        assert!(matches!(
            state,
            SpeechAudioState::Unavailable {
                reason: SpeechUnavailableReason::NotRequested,
                ..
            }
        ));
    }
}
