use super::*;
use crate::ai::graph_store::{self, Partition, TransactionStore};
use rusqlite::Connection;
use serde_json::json;

fn settings() -> Settings {
    Settings {
        target: ResolvedTarget {
            audio_resolution: Some(crate::configuration::speech::Resolution {
                requested_model: "requested".into(),
                model: "captured".into(),
                provider: "fixture".into(),
                language_code: "es".into(),
                reason: "configured".into(),
            }),
            route: crate::model::ConnectionRoute::Custom,
            revision: 2,
            url: "http://127.0.0.1:8765/v1/audio/speech".into(),
            model: "captured".into(),
            credential: Some("reference-only".into()),
        },
        install_id: "install".into(),
        language_tag: "es".into(),
        language: "Spanish".into(),
        voice: "voice".into(),
    }
}
pub(super) fn values() -> Values {
    capture(
        SourceText {
            id: "message".into(),
            text: "  cafe\u{301} العربية 日本語\n".into(),
        },
        settings(),
    )
    .unwrap()
}

#[test]
fn capture_preserves_source_and_routing_and_rejects_inconsistent_resolution() {
    let values = values();
    let captured = decode(&values).unwrap();
    assert_eq!(captured.speech_input().text, values["source"]["text"]);
    assert_eq!(
        captured
            .settings
            .target
            .audio_resolution
            .unwrap()
            .requested_model,
        "requested"
    );
    for path in [
        "model",
        "requested_model",
        "provider",
        "language_code",
        "reason",
    ] {
        let mut invalid = values.clone();
        invalid.get_mut("speech").unwrap()["target"]["audio_resolution"][path] = json!("");
        assert!(decode(&invalid).is_err());
    }
    let mut plain = settings();
    plain.target.audio_resolution = None;
    plain.target.credential = None;
    let captured = capture(captured.source, plain).unwrap();
    assert_eq!(
        captured["speech"]["target"]["audio_resolution"],
        json!(null)
    );
    assert!(
        decode(&captured)
            .unwrap()
            .settings
            .target
            .audio_resolution
            .is_none()
    );
    let mut invalid = values;
    invalid.get_mut("source").unwrap()["text"] = json!("bad\0source");
    assert!(decode(&invalid).is_err());
}

pub(super) fn adapter<'a>(
    db: &'a mut Connection,
    partition: &Partition,
) -> TransactionStore<'a, impl FnMut(&Connection, &CommitRequest<'_>) -> graph::Result<()>> {
    TransactionStore::new(
        db.transaction().unwrap(),
        partition.clone(),
        1_000_000,
        |_: &Connection, _: &CommitRequest<'_>| Ok(()),
    )
}

#[tokio::test]
async fn synthesis_requires_demand_retains_evidence_and_rejects_foreign_receipts() {
    for foreign in [false, true] {
        let graph = Arc::new(
            compile(Arc::new(move |invocation, request| {
                Box::pin(async move {
                    assert_eq!(request.source.text, "  cafe\u{301} العربية 日本語\n");
                    invocation.observe(ResponseEvidence {
                        request_id: Some("provider-request".into()),
                        actual_model: Some("served".into()),
                        ..Default::default()
                    })?;
                    Ok(Receipt {
                        id: "receipt".into(),
                        engine: if foreign {
                            "foreign".into()
                        } else {
                            invocation.identity().engine.clone().unwrap()
                        },
                        execution: serde_json::to_value(invocation.identity().execution)
                            .unwrap()
                            .to_string(),
                        digest: "a".repeat(64),
                        bytes: 100,
                    })
                })
            }))
            .unwrap(),
        );
        let mut db = Connection::open_in_memory().unwrap();
        db.execute_batch("PRAGMA foreign_keys=ON; CREATE TABLE conversations(id TEXT PRIMARY KEY); INSERT INTO conversations VALUES('conversation');").unwrap();
        db.execute_batch(include_str!("../../storage/schemas/graph_runtime.sql"))
            .unwrap();
        let partition = Partition {
            owner: crate::ai::graph_store::Owner::Conversation("conversation".into()),
            catalog: graph_store::catalog_id([graph.identity()]).unwrap(),
        };
        let mut engine = DurableEngine::create_with_run(
            [graph.clone()],
            crate::conversations::execution::graph_runtime::limits(),
            Event::Begin {
                run: "run".into(),
                artifact: graph.identity().into(),
                scope: "source-owner".into(),
                inputs: values(),
                policy: BTreeMap::new(),
            },
            &mut adapter(&mut db, &partition),
        )
        .unwrap();
        let capacity = Capacity {
            local: 0,
            provider: 1,
        };
        assert!(
            engine
                .apply(Event::Advance(capacity), &mut adapter(&mut db, &partition))
                .unwrap()
                .is_empty()
        );
        engine
            .apply(
                Event::Demand {
                    run: "run".into(),
                    node: "synthesize".into(),
                },
                &mut adapter(&mut db, &partition),
            )
            .unwrap();
        let work = engine
            .apply(Event::Advance(capacity), &mut adapter(&mut db, &partition))
            .unwrap()
            .remove(0);
        let attempt_id = engine.inspect("run").unwrap().attempts["synthesize"]
            .last()
            .unwrap()
            .id;
        let report = engine
            .claim(
                "run",
                "synthesize",
                attempt_id,
                &mut adapter(&mut db, &partition),
            )
            .unwrap()
            .execute_with_provisional(
                EvidenceLimits {
                    observations: 4,
                    bytes: 4096,
                },
                ProvisionalLimits { bytes: 4096 },
            )
            .await;
        engine
            .settle_report(&report, &mut adapter(&mut db, &partition))
            .unwrap();
        if foreign {
            assert!(matches!(
                engine.inspect("run").unwrap().attempts["synthesize"]
                    .last()
                    .unwrap()
                    .state,
                AttemptState::Failed(_)
            ));
        } else {
            engine
                .adopt(
                    "run",
                    "synthesize",
                    attempt_id,
                    &mut adapter(&mut db, &partition),
                )
                .unwrap();
            assert_eq!(
                engine.inspect("run").unwrap().nodes["synthesize"],
                Disposition::Adopted
            );
        }
        let mut reader = graph_store::ReadStore::new(&mut db, partition).unwrap();
        let evidence = engine
            .read_execution_evidence(work.execution, &mut reader)
            .unwrap()
            .unwrap();
        assert_eq!(
            evidence.observations[0].request_id.as_deref(),
            Some("provider-request")
        );
    }
}
