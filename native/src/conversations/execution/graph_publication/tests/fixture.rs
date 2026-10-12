use super::*;
use std::{collections::BTreeMap, sync::Arc};

pub(super) fn executable() -> Arc<Executable> {
    Arc::new(
        crate::conversations::execution::coach_graph::compile(Arc::new(|invocation, request| {
            Box::pin(async move {
                assert!(invocation.identity().engine.is_some());
                invocation.observe(ResponseEvidence {
                    request_id: Some("fixture-provider-request".into()),
                    actual_model: Some("fixture".into()),
                    ..Default::default()
                })?;
                Ok(request.context.messages.last().unwrap().content.clone())
            })
        }))
        .unwrap(),
    )
}

pub(super) fn inputs(text: &str) -> Values {
    use crate::ai::{connections::access::ResolvedTarget, transport::provider::PromptMessage};
    crate::conversations::execution::coach_graph::capture(
        vec![
            PromptMessage {
                role: "system".into(),
                content: "Fixture instructions".into(),
            },
            PromptMessage {
                role: "user".into(),
                content: text.into(),
            },
        ],
        vec!["user".into()],
        &ResolvedTarget {
            audio_resolution: None,
            route: crate::model::ConnectionRoute::Hosted,
            revision: 1,
            url: format!("{}/v1/operations", crate::ai::hosted::ORIGIN),
            model: "fixture".into(),
            credential: Some("reference".into()),
        },
        "fixture-install",
    )
    .unwrap()
}

pub(super) fn limits() -> DurableLimits {
    DurableLimits {
        record_reads: RecordReadLimits {
            records: 1000,
            bytes: 1_000_000,
        },
        state: StateLimits {
            runs: 10,
            attempts: 100,
            executions: 100,
        },
        checkpoint: CheckpointLimits {
            bytes: 100_000,
            events: 100,
        },
        settlement_event_bytes: 8192,
        history: HistoryLimits {
            bytes: 1_000_000,
            events: 1000,
            segments: 100,
        },
    }
}

pub(super) fn db() -> Connection {
    let db = Connection::open_in_memory().unwrap();
    db.execute_batch("PRAGMA foreign_keys=ON").unwrap();
    for schema in [
        include_str!("../../../../storage/schemas/schema.sql"),
        include_str!("../../../../storage/schemas/graph_runtime.sql"),
        include_str!("../../../../storage/schemas/turn_execution_owners.sql"),
        include_str!("../../../../storage/schemas/graph_publications.sql"),
        include_str!("../../../../storage/schemas/graph_reply_sources.sql"),
        include_str!("../../../../storage/schemas/graph_assessment_owners.sql"),
        include_str!("../../../../storage/schemas/graph_transport.sql"),
        include_str!("../../../../storage/schemas/graph_speech_requests.sql"),
        include_str!("../../../../storage/schemas/graph_helper_requests.sql"),
    ] {
        db.execute_batch(schema).unwrap();
    }
    db.execute_batch("INSERT INTO learner VALUES('learner',1,'Fixture',1,'{}'); INSERT INTO personas VALUES('p','learner','english',1,'{}'); INSERT INTO contacts VALUES('c','learner','p',0,1); INSERT INTO conversations VALUES('conversation','c','english','Fixture',0,1,'then',1);
        INSERT INTO turns(id,conversation_id,state,paused,profile_revision,credential_id,route,model,context) VALUES('turn','conversation','pending',0,1,'reference','hosted','fixture','{\"practiceSettings\":{\"varietyId\":\"captured-variety\"}}');
        INSERT INTO messages(id,conversation_id,turn_id,sequence,role,text) VALUES('user','conversation','turn',1,'user','Question');").unwrap();
    db.execute(
        "UPDATE ai_config SET hosted_credential_id='reference',standard_model='fixture'",
        [],
    )
    .unwrap();
    db
}

/// The host retains full domain diagnostics beside the core's bounded rejection
/// signal. This fixture does not stand in for production access/dispatch wiring.
pub(super) fn store<'a>(
    db: &'a mut Connection,
    partition: &Partition,
    error: &'a mut Option<AppError>,
    deny: bool,
    budget: usize,
) -> TransactionStore<'a, impl FnMut(&Connection, &CommitRequest<'_>) -> graph::Result<()> + 'a> {
    let callback = move |db: &Connection, request: &CommitRequest<'_>| {
        let outcome = (|| -> crate::model::Result<()> {
            match &request.intent {
                CommitIntent::Begin { authority, .. } => {
                    db.execute(
                        "INSERT INTO turn_execution_owners VALUES('turn','graph','coach',?1,?2,?3)",
                        params![
                            request.next.stamp().engine,
                            authority.run,
                            authority.artifact
                        ],
                    )?;
                    declare_reply(db, request, "reply", "text")?;
                }
                CommitIntent::Dispatch { work, .. } if work.resource == Resource::Provider => {
                    crate::conversations::execution::graph_authority::dispatch(db, request)?;
                    let identity = crate::ai::transport::graph_identity::bind(db, request)?;
                    assert_eq!(
                        identity,
                        crate::ai::transport::graph_identity::bind(db, request)?
                    );
                }
                CommitIntent::Adopt { authority, .. } if authority.node == Some("reply") => {
                    publish_reply(db, request, |_, _| {
                        if deny {
                            Err(AppError::new(ErrorCode::Conflict, "Access revoked."))
                        } else {
                            Ok(())
                        }
                    })?
                }
                _ => (),
            }
            Ok(())
        })();
        outcome.map_err(|cause| {
            *error = Some(cause);
            Fault {
                code: "domain_publication_rejected".into(),
                path: "publication".into(),
            }
        })
    };
    TransactionStore::new(
        db.transaction().unwrap(),
        partition.clone(),
        budget,
        callback,
    )
}

pub(super) async fn prepared(text: &str) -> (Connection, Partition, DurableEngine, AttemptId) {
    let mut db = db();
    let graph = executable();
    let partition = Partition {
        owner: crate::ai::graph_store::Owner::Conversation("conversation".into()),
        catalog: catalog_id([graph.identity()]).unwrap(),
    };
    let mut error = None;
    let mut host = DurableEngine::create_with_run(
        [graph.clone()],
        limits(),
        Event::Begin {
            run: "run".into(),
            artifact: graph.identity().into(),
            inputs: inputs(text),
            scope: "captured-authority".into(),
            policy: BTreeMap::new(),
        },
        &mut store(&mut db, &partition, &mut error, false, 100_000),
    )
    .unwrap();
    host.apply(
        Event::Advance(Capacity {
            local: 1,
            provider: 0,
        }),
        &mut store(&mut db, &partition, &mut error, false, 100_000),
    )
    .unwrap();
    let context_attempt = host.inspect("run").unwrap().attempts["context"][0].id;
    let report = host
        .claim(
            "run",
            "context",
            context_attempt,
            &mut store(&mut db, &partition, &mut error, false, 100_000),
        )
        .unwrap()
        .execute(EvidenceLimits {
            observations: 10,
            bytes: 8192,
        })
        .await;
    host.apply(
        Event::SettleObserved(report),
        &mut store(&mut db, &partition, &mut error, false, 100_000),
    )
    .unwrap();
    host.adopt(
        "run",
        "context",
        context_attempt,
        &mut store(&mut db, &partition, &mut error, false, 100_000),
    )
    .unwrap();
    assert_eq!(count(&db, "conversation_graph_publications"), 0);
    host.apply(
        Event::Advance(Capacity {
            local: 1,
            provider: 1,
        }),
        &mut store(&mut db, &partition, &mut error, false, 100_000),
    )
    .unwrap();
    let attempt = host.inspect("run").unwrap().attempts["reply"][0].id;
    assert!(error.is_none());
    (db, partition, host, attempt)
}

pub(super) async fn available(text: &str) -> (Connection, Partition, DurableEngine, AttemptId) {
    let (mut db, partition, mut host, attempt) = prepared(text).await;
    let mut error = None;
    let invocation = host
        .claim(
            "run",
            "reply",
            attempt,
            &mut store(&mut db, &partition, &mut error, false, 100_000),
        )
        .unwrap();
    let report = invocation
        .execute(EvidenceLimits {
            observations: 10,
            bytes: 8192,
        })
        .await;
    host.apply(
        Event::SettleObserved(report),
        &mut store(&mut db, &partition, &mut error, false, 100_000),
    )
    .unwrap();
    assert!(error.is_none());
    (db, partition, host, attempt)
}

pub(super) fn count(db: &Connection, table: &str) -> i64 {
    db.query_row(&format!("SELECT count(*) FROM {table}"), [], |r| r.get(0))
        .unwrap()
}
