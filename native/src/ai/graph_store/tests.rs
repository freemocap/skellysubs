use super::*;
use std::{collections::BTreeMap, sync::Arc};

fn graph() -> Arc<Executable> {
    let number = Contract::new("number", 1);
    let ports = BTreeMap::from([(
        "value".into(),
        Port {
            contract: number.clone(),
            optional: false,
        },
    )]);
    let mut registry = Registry::default();
    registry.define_type(number, Shape::Integer).unwrap();
    registry
        .register(
            Operation {
                contract: Contract::new("echo", 1),
                implementation: "fixture/echo/1".into(),
                inputs: ports.clone(),
                outputs: ports.clone(),
                resource: Resource::Local,
                reuse: Reuse::Exact,
            },
            Arc::new(|_, values| Box::pin(async move { Ok(values) })),
        )
        .unwrap();
    Arc::new(
        registry
            .compile(Definition {
                contract: Contract::new("fixture", 1),
                inputs: ports.clone(),
                outputs: ports,
                nodes: BTreeMap::from([(
                    "echo".into(),
                    Node {
                        operation: Contract::new("echo", 1),
                        inputs: BTreeMap::from([("value".into(), Source::Input("value".into()))]),
                        after: vec![],
                        guard: None,
                        activation: Activation::Automatic,
                    },
                )]),
                results: BTreeMap::from([(
                    "value".into(),
                    Source::Output {
                        node: "echo".into(),
                        port: "value".into(),
                    },
                )]),
                compositions: BTreeMap::new(),
            })
            .unwrap(),
    )
}
fn limits() -> DurableLimits {
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
fn db() -> Connection {
    let db = Connection::open_in_memory().unwrap();
    db.execute_batch("PRAGMA foreign_keys=ON; CREATE TABLE conversations(id TEXT PRIMARY KEY); INSERT INTO conversations VALUES('one'),('two'); CREATE TABLE accepted(value TEXT);").unwrap();
    db.execute_batch(include_str!("../../storage/schemas/graph_runtime.sql"))
        .unwrap();
    db
}
fn partition(graph: &Executable, conversation: &str) -> Partition {
    Partition {
        owner: crate::ai::graph_store::Owner::Conversation(conversation.into()),
        catalog: catalog_id([graph.identity()]).unwrap(),
    }
}
fn begin(graph: &Executable) -> Event {
    Event::Begin {
        run: "run".into(),
        artifact: graph.identity().into(),
        inputs: BTreeMap::from([("value".into(), serde_json::json!(7))]),
        scope: "authorized".into(),
        policy: BTreeMap::new(),
    }
}
fn owner(db: &Connection, request: &CommitRequest<'_>) -> Result<()> {
    match &request.intent {
        CommitIntent::Begin { authority, .. }
        | CommitIntent::Dispatch { authority, .. }
        | CommitIntent::Adopt { authority, .. }
            if authority.scope != "authorized" =>
        {
            return Err(fault("revoked", "owner"));
        }
        CommitIntent::Adopt { values, .. } => {
            db.execute(
                "INSERT INTO accepted VALUES(?1)",
                [encode(values, "value")?],
            )
            .map_err(|e| sql(e, "publication"))?;
        }
        _ => (),
    }
    Ok(())
}
type FixtureStore<'a> = TransactionStore<'a, fn(&Connection, &CommitRequest<'_>) -> Result<()>>;
fn store<'a>(db: &'a mut Connection, p: &Partition) -> FixtureStore<'a> {
    TransactionStore::new(db.transaction().unwrap(), p.clone(), 100_000, owner)
}

#[test]
fn first_admission_binds_turn_to_actual_native_engine_in_the_same_commit() {
    for reject in [false, true] {
        let mut db = db();
        let graph = graph();
        let p = partition(&graph, "one");
        db.execute_batch("CREATE TABLE turns(id TEXT PRIMARY KEY,conversation_id TEXT REFERENCES conversations(id));").unwrap();
        db.execute_batch(include_str!(
            "../../storage/schemas/turn_execution_owners.sql"
        ))
        .unwrap();
        let tx = db.transaction().unwrap();
        tx.execute("INSERT INTO turns VALUES('turn','one')", [])
            .unwrap();
        let callback = |db: &Connection, request: &CommitRequest<'_>| -> Result<()> {
            let CommitIntent::Begin { authority, .. } = &request.intent else {
                return Err(fault("unexpected_intent", "fixture"));
            };
            db.execute(
                "INSERT INTO turn_execution_owners VALUES('turn','graph','coach',?1,?2,?3)",
                params![
                    request.next.stamp().engine,
                    authority.run,
                    authority.artifact
                ],
            )
            .map_err(|e| sql(e, "owner"))?;
            Ok(())
        };
        let mut adapter = TransactionStore::new(tx, p, if reject { 0 } else { 100_000 }, callback);
        let result =
            DurableEngine::create_with_run([graph.clone()], limits(), begin(&graph), &mut adapter);
        drop(adapter);
        assert_eq!(result.is_err(), reject);
        if reject {
            for table in ["turns", "turn_execution_owners", "graph_engines"] {
                assert_eq!(
                    db.query_row(&format!("SELECT count(*) FROM {table}"), [], |r| r
                        .get::<_, i64>(0))
                        .unwrap(),
                    0
                );
            }
        } else {
            let host = result.unwrap();
            let owner: String = db
                .query_row(
                    "SELECT engine_id FROM turn_execution_owners WHERE turn_id='turn'",
                    [],
                    |r| r.get(0),
                )
                .unwrap();
            assert_eq!(&owner, &host.stamp().engine);
            crate::conversations::execution_owner::validate_complete(&db).unwrap();
        }
    }
}

#[tokio::test]
async fn native_lifecycle_persists_compacts_and_recovers_through_sql_adapter() {
    let mut db = db();
    let graph = graph();
    let p = partition(&graph, "one");
    let mut host = DurableEngine::create_with_run(
        [graph.clone()],
        limits(),
        begin(&graph),
        &mut store(&mut db, &p),
    )
    .unwrap();
    host.apply(
        Event::Advance(Capacity {
            local: 1,
            provider: 0,
        }),
        &mut store(&mut db, &p),
    )
    .unwrap();
    let attempt = host.inspect("run").unwrap().attempts["echo"][0].id;
    let invocation = host
        .claim("run", "echo", attempt, &mut store(&mut db, &p))
        .unwrap();
    let report = invocation
        .execute(EvidenceLimits {
            observations: 10,
            bytes: 8192,
        })
        .await;
    host.apply(Event::SettleObserved(report), &mut store(&mut db, &p))
        .unwrap();
    host.adopt("run", "echo", attempt, &mut store(&mut db, &p))
        .unwrap();
    host.evict_records(&mut store(&mut db, &p)).unwrap();
    assert_eq!(
        db.query_row("SELECT count(*) FROM accepted", [], |r| r.get::<_, i64>(0))
            .unwrap(),
        1
    );
    assert!(
        db.execute("UPDATE graph_archives SET payload=x'00'", [])
            .is_err()
    );
    let checkpoint = {
        let mut reader = ReadStore::new(&mut db, p.clone()).unwrap();
        assert_eq!(reader.record_count(host.stamp()).unwrap(), 3);
        let snapshot = host
            .read_inspection(
                "run",
                ExportLimits {
                    bytes: 100_000,
                    attempts: 100,
                },
                &mut reader,
            )
            .unwrap();
        assert_eq!(snapshot.nodes["echo"], Disposition::Adopted);
        reader.checkpoint(limits().checkpoint).unwrap().unwrap()
    };
    let recovered =
        DurableEngine::recover(checkpoint, [graph], limits(), &mut store(&mut db, &p)).unwrap();
    assert_eq!(recovered.outputs("run").unwrap().unwrap()["value"], 7);
}

#[test]
fn rollback_identity_bounds_and_corruption_fail_without_fallback() {
    let mut db = db();
    let graph = graph();
    let p = partition(&graph, "one");
    {
        let tx = db.transaction().unwrap();
        tx.execute("INSERT INTO accepted VALUES('staged')", [])
            .unwrap();
        let mut adapter = TransactionStore::new(tx, p.clone(), 0, owner);
        assert!(
            DurableEngine::create_with_run([graph.clone()], limits(), begin(&graph), &mut adapter)
                .is_err()
        );
    }
    assert_eq!(
        db.query_row("SELECT count(*) FROM graph_engines", [], |r| r
            .get::<_, i64>(0))
            .unwrap(),
        0
    );
    assert_eq!(
        db.query_row("SELECT count(*) FROM accepted", [], |r| r.get::<_, i64>(0))
            .unwrap(),
        0
    );
    let host = DurableEngine::create_with_run(
        [graph.clone()],
        limits(),
        begin(&graph),
        &mut store(&mut db, &p),
    )
    .unwrap();
    assert!(
        DurableEngine::create_with_run(
            [graph.clone()],
            limits(),
            begin(&graph),
            &mut store(&mut db, &p)
        )
        .is_err()
    );
    {
        let mut wrong = ReadStore::new(&mut db, partition(&graph, "two")).unwrap();
        assert!(wrong.checkpoint(limits().checkpoint).unwrap().is_none());
        assert!(wrong.record_count(host.stamp()).is_err());
    }
    {
        let mut reader = ReadStore::new(&mut db, p.clone()).unwrap();
        assert!(
            reader
                .read_record(host.stamp(), &RecordKey::Run("run".into()), 0)
                .is_err()
        );
        assert!(
            reader
                .checkpoint(CheckpointLimits {
                    bytes: 1,
                    events: 100
                })
                .is_err()
        );
        let mut stale = host.stamp().clone();
        stale.revision += 1;
        assert!(reader.record_count(&stale).is_err());
    }
    db.execute("UPDATE graph_records SET payload=?1", [b"{}".to_vec()])
        .unwrap();
    let mut reader = ReadStore::new(&mut db, p).unwrap();
    assert!(
        host.read_inspection(
            "run",
            ExportLimits {
                bytes: 100_000,
                attempts: 100
            },
            &mut reader
        )
        .is_err()
    );
}

#[test]
fn catalog_and_conversation_ownership_are_enforced_and_delete_is_scoped() {
    assert_eq!(
        catalog_id(["a", "b"]).unwrap(),
        catalog_id(["b", "a"]).unwrap()
    );
    assert!(catalog_id(["a", "a"]).is_err());
    let mut db = db();
    let graph = graph();
    for conversation in ["one", "two"] {
        let p = partition(&graph, conversation);
        let mut denied = begin(&graph);
        if let Event::Begin { scope, .. } = &mut denied {
            *scope = "revoked".into();
        }
        assert!(
            DurableEngine::create_with_run(
                [graph.clone()],
                limits(),
                denied,
                &mut store(&mut db, &p)
            )
            .is_err()
        );
        let mut host = DurableEngine::create_with_run(
            [graph.clone()],
            limits(),
            begin(&graph),
            &mut store(&mut db, &p),
        )
        .unwrap();
        host.evict_records(&mut store(&mut db, &p)).unwrap();
    }
    assert!(
        db.execute(
            "UPDATE graph_engines SET conversation_id='two' WHERE conversation_id='one'",
            []
        )
        .is_err()
    );
    db.execute("DELETE FROM conversations WHERE id='one'", [])
        .unwrap();
    for table in ["graph_engines", "graph_records", "graph_archives"] {
        assert_eq!(
            db.query_row(&format!("SELECT count(*) FROM {table}"), [], |r| r
                .get::<_, i64>(0))
                .unwrap(),
            1
        );
    }
    assert!(
        ReadStore::new(&mut db, partition(&graph, "two"))
            .unwrap()
            .checkpoint(limits().checkpoint)
            .unwrap()
            .is_some()
    );
}

#[test]
fn invalid_storage_authority_is_rejected_and_sql_errors_keep_only_safe_codes() {
    let mut db = db();
    let graph = graph();
    let mut p = partition(&graph, "one");
    p.catalog = catalog_id(["other-artifact"]).unwrap();
    assert_eq!(
        DurableEngine::create_with_run(
            [graph.clone()],
            limits(),
            begin(&graph),
            &mut store(&mut db, &p)
        )
        .err()
        .unwrap()
        .code,
        "artifact_mismatch"
    );
    p = partition(&graph, "missing-conversation");
    let failure = DurableEngine::create_with_run(
        [graph.clone()],
        limits(),
        begin(&graph),
        &mut store(&mut db, &p),
    )
    .err()
    .unwrap();
    assert_eq!(failure.code, "graph_storage_sqlite_19_787");
    assert_eq!(failure.path, "create");
    assert!(!format!("{failure:?}").contains("missing-conversation"));
    db.pragma_update(None, "foreign_keys", false).unwrap();
    p = partition(&graph, "one");
    assert_eq!(
        DurableEngine::create_with_run(
            [graph.clone()],
            limits(),
            begin(&graph),
            &mut store(&mut db, &p)
        )
        .err()
        .unwrap()
        .code,
        "graph_storage_foreign_keys_required"
    );
    assert_eq!(
        db.query_row("SELECT count(*) FROM graph_engines", [], |r| r
            .get::<_, i64>(0))
            .unwrap(),
        0
    );
}

#[test]
fn workspace_partition_uses_same_native_commit_protocol_without_conversation() {
    let mut db = db();
    db.execute_batch(
        "CREATE TABLE learner(id TEXT PRIMARY KEY); INSERT INTO learner VALUES('workspace');",
    )
    .unwrap();
    db.execute_batch(include_str!(
        "../../storage/schemas/workspace_graph_runtime.sql"
    ))
    .unwrap();
    let graph = graph();
    let partition = Partition {
        owner: Owner::Workspace("workspace".into()),
        catalog: catalog_id([graph.identity()]).unwrap(),
    };
    let mut adapter = TransactionStore::new(
        db.transaction().unwrap(),
        partition.clone(),
        limits().checkpoint.bytes,
        |_: &Connection, _: &CommitRequest<'_>| Ok(()),
    );
    let engine =
        DurableEngine::create_with_run([graph.clone()], limits(), begin(&graph), &mut adapter)
            .unwrap();
    drop(adapter);
    assert_eq!(
        db.query_row("SELECT count(*) FROM graph_engines", [], |r| r
            .get::<_, i64>(0))
            .unwrap(),
        0
    );
    let reader = ReadStore::new(&mut db, partition).unwrap();
    assert_eq!(
        reader
            .checkpoint(limits().checkpoint)
            .unwrap()
            .unwrap()
            .stamp(),
        engine.stamp()
    );
}

#[test]
fn detached_history_preserves_its_cut_after_source_changes_and_close() {
    let mut db = db();
    let graph = graph();
    let p = partition(&graph, "one");
    let mut adapter = store(&mut db, &p);
    let engine =
        DurableEngine::create_with_run([graph.clone()], limits(), begin(&graph), &mut adapter)
            .unwrap();
    let expected = engine.stamp().clone();
    drop(adapter);
    let tx = db.unchecked_transaction().unwrap();
    let mut detached =
        DetachedHistory::capture(&tx, &p, limits().checkpoint.bytes, limits().history).unwrap();
    drop(tx);
    db.execute("UPDATE graph_engines SET checkpoint=x'00'", [])
        .unwrap();
    let tx = db.unchecked_transaction().unwrap();
    let corrupt =
        DetachedHistory::capture(&tx, &p, limits().checkpoint.bytes, limits().history).unwrap();
    assert_ne!(
        detached.cache_identity().unwrap(),
        corrupt.cache_identity().unwrap()
    );
    assert!(corrupt.checkpoint(limits().checkpoint).is_err());
    drop(tx);
    drop(db);
    let checkpoint = detached.checkpoint(limits().checkpoint).unwrap();
    assert_eq!(checkpoint.stamp(), &expected);
    let cut = checkpoint
        .historical_inspection(
            expected.revision,
            HistoricalLimits {
                history: limits().history,
                state: limits().state,
            },
            &mut detached,
        )
        .unwrap();
    assert_eq!(
        cut.snapshot(
            "run",
            ExportLimits {
                bytes: 100_000,
                attempts: 100
            }
        )
        .unwrap()
        .run,
        "run"
    );
}

#[test]
fn detached_history_requires_transaction_and_bounds_before_copying() {
    let mut db = db();
    let graph = graph();
    let p = partition(&graph, "one");
    let mut adapter = store(&mut db, &p);
    DurableEngine::create_with_run([graph.clone()], limits(), begin(&graph), &mut adapter).unwrap();
    drop(adapter);
    assert!(
        DetachedHistory::capture(&db, &p, limits().checkpoint.bytes, limits().history).is_err()
    );
    let tx = db.unchecked_transaction().unwrap();
    assert!(DetachedHistory::capture(&tx, &p, 1, limits().history).is_err());
}
