//! Actual speech commands after eviction/restart, with controlled audio providers.
use super::*;
fn request(store: &Store, message: &str) -> Command {
    Command {
        session_id: store.session_id.clone(),
        action_id: id(),
        action: Action::RequestMessageSpeech {
            regenerate: None,
            message_id: message.into(),
        },
    }
}
pub(super) async fn exercise(store: &mut Store, message: &str) {
    let (speech_tx, mut speech_rx) = tokio::sync::mpsc::unbounded_channel();
    let (lookup_tx, mut lookup_rx) = tokio::sync::mpsc::unbounded_channel();
    store.graph_runtime.partner.bind_speech(
        Arc::new(move |ctx, _| {
            let (tx, rx) = tokio::sync::oneshot::channel();
            speech_tx.send((ctx, tx)).unwrap();
            Box::pin(async move { rx.await.unwrap() })
        }),
        Arc::new(move |ctx, _| {
            let (tx, rx) = tokio::sync::oneshot::channel();
            lookup_tx.send((ctx, tx)).unwrap();
            Box::pin(async move { rx.await.unwrap() })
        }),
    );
    let first = request(store, message);
    let cancelled = store.execute(first.clone()).unwrap().entity_id;
    assert_eq!(store.execute(first).unwrap().entity_id, cancelled);
    store
        .execute(Command {
            session_id: store.session_id.clone(),
            action_id: id(),
            action: Action::CancelMessageSpeech {
                operation_id: cancelled.clone(),
            },
        })
        .unwrap();
    assert!(matches!(
        store.speech_audio(&cancelled).unwrap(),
        SpeechAudioState::Unavailable {
            reason: SpeechUnavailableReason::Cancelled,
            ..
        }
    ));
    crate::ai::results::set_capacity(&store.connection, 1_000_000).unwrap();
    let mut generated = 0;
    for expected in 1..=5 {
        let mut command = request(store, message);
        if expected == 3 {
            command.action = Action::RequestMessageSpeech {
                message_id: message.into(),
                regenerate: Some(true),
            };
        }
        let operation = store.execute(command).unwrap().entity_id;
        assert_ne!(operation, cancelled);
        let mut ready = None;
        for _ in 0..50 {
            if let SpeechAudioState::Ready {
                attempt_id,
                audio_base64,
                alignment,
                ..
            } = store.speech_audio(&operation).unwrap()
            {
                ready = Some((
                    attempt_id,
                    crate::speech::alignment::SpeechAudio {
                        audio_base64,
                        alignment,
                    },
                ));
                break;
            }
            if expected == 4
                && matches!(
                    store.speech_audio(&operation).unwrap(),
                    SpeechAudioState::Unavailable {
                        reason: SpeechUnavailableReason::Failed,
                        ..
                    }
                )
            {
                break;
            }
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
            let mut task = tokio::spawn(claim.invocation.execute(EvidenceLimits {
                observations: 16,
                bytes: 65536,
            }));
            let report = tokio::select! {
                result = &mut task => result.unwrap(),
                Some((context,answer)) = lookup_rx.recv() => {
                    let cached = store.graph_runtime.lookup_audio(&store.connection,context.identity()).unwrap();
                    answer.send(Ok(cached)).unwrap();task.await.unwrap()
                },
                Some((context,answer)) = speech_rx.recv() => {
                    generated += 1;
                    if expected == 4 {
                        answer.send(Err(Fault{code:"fixture_speech_failure".into(),path:"provider".into()})).unwrap();
                        task.await.unwrap()
                    } else {
                    let mut bytes = std::io::Cursor::new(Vec::new());
                    { let mut wav = hound::WavWriter::new(&mut bytes,hound::WavSpec{channels:1,sample_rate:24000,bits_per_sample:16,sample_format:hound::SampleFormat::Int}).unwrap();wav.write_sample(42i16).unwrap();wav.finalize().unwrap(); }
                    let receipt = store.graph_runtime.stage_audio(&store.connection,context.identity(),bytes.into_inner(),None).unwrap();
                    answer.send(Ok(receipt)).unwrap();task.await.unwrap()
                    }
                }
            };
            assert_eq!(
                report.outcome.is_ok(),
                expected != 4
                    || report.identity.operation
                        != crate::speech::synthesis_graph::operation_contract(),
                "{:?}",
                report.outcome
            );
            store
                .graph_runtime
                .finish(
                    &mut store.connection,
                    &claim.conversation,
                    &claim.run,
                    report,
                )
                .unwrap();
        }
        if expected == 4 {
            assert!(ready.is_none());
            assert_eq!(generated, 4);
            assert!(
                store
                    .graph_runtime
                    .next(
                        &mut store.connection,
                        true,
                        &store.config,
                        &store.session_id
                    )
                    .unwrap()
                    .is_none()
            );
            continue;
        }
        let (attempt, audio) = ready.expect("requested speech never delivered");
        assert_eq!(generated, expected);
        assert!(
            store
                .delivered_speech_owner(&operation, &attempt, &audio)
                .is_ok()
        );
        let reused = store.execute(request(store, message)).unwrap().entity_id;
        assert!(matches!(
            store.speech_audio(&reused).unwrap(),
            SpeechAudioState::Ready { .. }
        ));
        let count: i64 = store
            .connection
            .query_row("SELECT count(*) FROM graph_audio_receipts", [], |r| {
                r.get(0)
            })
            .unwrap();
        assert_eq!(count, expected + i64::from(expected < 4));
        let product = store
            .conversation_snapshot(
                &store
                    .connection
                    .query_row(
                        "SELECT conversation_id FROM messages WHERE id=?1",
                        [message],
                        |r| r.get::<_, String>(0),
                    )
                    .unwrap(),
                None,
            )
            .unwrap();
        assert!(
            product
                .turns
                .iter()
                .filter_map(|t| t.speech.as_ref())
                .any(|o| o.source_message_id == message)
        );
        if expected == 2 {
            continue;
        }
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
                .delivered_speech_owner(&operation, &attempt, &audio)
                .is_ok()
        );
        crate::ai::results::set_capacity(&store.connection, 1_000_000).unwrap();
        if expected == 5 {
            store.graph_runtime = graph_runtime::Runtime::new().unwrap();
            store.graph_runtime.recover(&mut store.connection).unwrap();
            assert!(matches!(
                store.speech_audio(&operation).unwrap(),
                SpeechAudioState::Unavailable {
                    reason: SpeechUnavailableReason::Expired,
                    ..
                }
            ));
            assert!(
                store
                    .delivered_speech_owner(&operation, &attempt, &audio)
                    .is_ok()
            );
        }
    }
    assert_eq!(
        store
            .connection
            .query_row(
                "SELECT count(*) FROM operations WHERE kind='persona_speech'",
                [],
                |r| r.get::<_, i64>(0)
            )
            .unwrap(),
        0
    );
}
