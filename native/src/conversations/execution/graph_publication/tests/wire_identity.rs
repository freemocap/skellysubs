use super::*;
use crate::ai::transport::graph_identity;

#[tokio::test]
async fn dispatch_rolls_back_wire_identity_with_records_then_reuses_committed_binding() {
    let (mut db, partition, mut host, attempt) = prepared("Reply").await;
    assert_eq!(
        count(&db, "graph_transport_identities"),
        0,
        "local work has no wire ID"
    );
    let before = host.stamp().clone();
    let mut error = None;
    assert!(
        host.claim(
            "run",
            "reply",
            attempt,
            &mut store(&mut db, &partition, &mut error, false, 0)
        )
        .is_err()
    );
    assert!(
        error.is_none(),
        "binding succeeded before record-write rejection"
    );
    assert_eq!(host.stamp(), &before);
    assert_eq!(count(&db, "graph_transport_identities"), 0);
    let report = host
        .claim(
            "run",
            "reply",
            attempt,
            &mut store(&mut db, &partition, &mut error, false, 100_000),
        )
        .unwrap()
        .execute(EvidenceLimits {
            observations: 10,
            bytes: 8192,
        })
        .await;
    let identity = graph_identity::load(&db, &report.identity).unwrap();
    assert_eq!(
        identity,
        graph_identity::load(&db, &report.identity).unwrap()
    );
    assert_eq!(identity.attempt.len(), 43);
    assert!(identity.attempt[..10].bytes().all(|b| b.is_ascii_digit()));
    assert!(uuid::Uuid::parse_str(&identity.attempt[11..]).is_ok());
    assert!(uuid::Uuid::parse_str(&identity.operation).is_ok());
    let mut foreign = report.identity.clone();
    foreign.artifact = "0".repeat(64);
    assert!(graph_identity::load(&db, &foreign).is_err());
    foreign = report.identity.clone();
    foreign.operation = Contract::new("other-operation", 1);
    assert!(graph_identity::load(&db, &foreign).is_err());
    foreign = report.identity.clone();
    foreign.engine = None;
    assert!(graph_identity::load(&db, &foreign).is_err());
    assert!(
        db.execute(
            "UPDATE graph_transport_identities SET operation_id=?1",
            ["a".repeat(32)]
        )
        .is_err()
    );
    assert_eq!(
        graph_identity::load(&db, &report.identity).unwrap(),
        identity
    );
    assert_eq!(count(&db, "graph_transport_identities"), 1);
    db.execute("DELETE FROM conversations", []).unwrap();
    assert_eq!(count(&db, "graph_transport_identities"), 0);
}

#[tokio::test]
async fn recovery_preserves_wire_identity_and_explicit_retry_gets_new_producer_ids() {
    let (mut db, partition, mut host, attempt) = prepared("Reply").await;
    let mut error = None;
    let invocation = host
        .claim(
            "run",
            "reply",
            attempt,
            &mut store(&mut db, &partition, &mut error, false, 100_000),
        )
        .unwrap();
    let old = InvocationIdentity {
        engine: Some(host.stamp().engine.clone()),
        execution: host.inspect("run").unwrap().attempts["reply"][0].execution,
        artifact: executable().identity().into(),
        operation: crate::conversations::execution::prose::operation_contract(),
    };
    let identity = graph_identity::load(&db, &old).unwrap();
    drop(invocation);
    drop(host);
    let checkpoint = ReadStore::new(&mut db, partition.clone())
        .unwrap()
        .checkpoint(limits().checkpoint)
        .unwrap()
        .unwrap();
    let mut host = DurableEngine::recover(
        checkpoint,
        [executable()],
        limits(),
        &mut store(&mut db, &partition, &mut error, false, 100_000),
    )
    .unwrap();
    assert_eq!(graph_identity::load(&db, &old).unwrap(), identity);
    assert_eq!(
        host.inspect("run").unwrap().nodes["reply"],
        Disposition::Unknown
    );
    host.apply(
        Event::Pause {
            run: "run".into(),
            paused: false,
        },
        &mut store(&mut db, &partition, &mut error, false, 100_000),
    )
    .unwrap();
    assert!(
        host.apply(
            Event::Advance(Capacity {
                local: 1,
                provider: 1
            }),
            &mut store(&mut db, &partition, &mut error, false, 100_000)
        )
        .unwrap()
        .is_empty()
    );
    assert_eq!(count(&db, "graph_transport_identities"), 1);
    host.apply(
        Event::Retry {
            run: "run".into(),
            node: "reply".into(),
        },
        &mut store(&mut db, &partition, &mut error, false, 100_000),
    )
    .unwrap();
    host.apply(
        Event::Advance(Capacity {
            local: 1,
            provider: 1,
        }),
        &mut store(&mut db, &partition, &mut error, false, 100_000),
    )
    .unwrap();
    let retry = host.inspect("run").unwrap().attempts["reply"]
        .last()
        .unwrap()
        .id;
    let report = host
        .claim(
            "run",
            "reply",
            retry,
            &mut store(&mut db, &partition, &mut error, false, 100_000),
        )
        .unwrap()
        .execute(EvidenceLimits {
            observations: 10,
            bytes: 8192,
        })
        .await;
    let retried = graph_identity::load(&db, &report.identity).unwrap();
    assert_ne!(report.identity.execution, old.execution);
    assert_ne!(retried.attempt, identity.attempt);
    assert_ne!(retried.operation, identity.operation);
    assert_eq!(graph_identity::load(&db, &old).unwrap(), identity);
    assert_eq!(count(&db, "graph_transport_identities"), 2);
}

#[tokio::test]
async fn schema_rejects_noncanonical_execution_ids_and_wire_ids() {
    let (db, _, host, _) = available("Reply").await;
    let artifact = executable().identity().to_owned();
    let insert = |execution: &str, attempt: &str, operation: &str| {
        db.execute(
            "INSERT INTO graph_transport_identities VALUES(?1,?2,?3,?4,?5,?6)",
            params![host.stamp().engine, execution, artifact, serde_json::to_string(&crate::conversations::execution::prose::operation_contract()).unwrap(), attempt, operation],
        )
    };
    let attempt = format!("1234567890-{}", "a".repeat(32));
    let operation = "b".repeat(32);
    for execution in ["0", "01", "-1", "18446744073709551616", "3\0"] {
        assert!(insert(execution, &attempt, &operation).is_err());
    }
    for bad in [
        "bad".to_string(),
        format!("1234567890-{}", "g".repeat(32)),
        format!("{attempt}\0"),
    ] {
        assert!(insert("999", &bad, &operation).is_err());
    }
    assert!(insert("999", &attempt, "bad").is_err());
    insert("18446744073709551615", &attempt, &operation).unwrap();
    assert!(
        insert("999", &attempt, &operation).is_err(),
        "wire IDs are unique"
    );
}
