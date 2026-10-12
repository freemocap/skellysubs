use super::*;
use crate::ai::graph::{self, *};
use crate::ai::graph_store::{Partition, ReadStore, TransactionStore, catalog_id};
mod fixture;
use fixture::*;
mod authority;
mod helpers;
mod reply_channels;
mod wire_identity;

#[tokio::test]
async fn interrupted_coach_producer_recovers_unknown_without_a_second_dispatch() {
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
    // A claimed request may have reached the provider. Dropping the host cannot
    // turn that uncertainty into a retry or a successful conversation result.
    drop(invocation);
    drop(host);
    let checkpoint = ReadStore::new(&mut db, partition.clone())
        .unwrap()
        .checkpoint(limits().checkpoint)
        .unwrap()
        .unwrap();
    let mut recovered = DurableEngine::recover(
        checkpoint,
        [executable()],
        limits(),
        &mut store(&mut db, &partition, &mut error, false, 100_000),
    )
    .unwrap();
    assert_eq!(
        recovered.inspect("run").unwrap().nodes["reply"],
        Disposition::Unknown
    );
    assert_eq!(
        recovered.inspect("run").unwrap().nodes["context"],
        Disposition::Adopted
    );
    assert!(
        recovered
            .apply(
                Event::Advance(Capacity {
                    local: 1,
                    provider: 1
                }),
                &mut store(&mut db, &partition, &mut error, false, 100_000)
            )
            .unwrap()
            .is_empty()
    );
    assert!(recovered.outputs("run").unwrap().is_none());
    assert_eq!(count(&db, "conversation_graph_publications"), 0);
    assert_eq!(count(&db, "effort_awards"), 0);
    assert_eq!(count(&db, "operations"), 0);
}

#[tokio::test]
async fn cancelled_coach_result_cannot_publish_or_award_credit() {
    let (mut db, partition, mut host, attempt) = available("Reply").await;
    let mut error = None;
    host.apply(
        Event::Cancel {
            run: "run".into(),
            node: Some("reply".into()),
        },
        &mut store(&mut db, &partition, &mut error, false, 100_000),
    )
    .unwrap();
    assert!(
        host.adopt(
            "run",
            "reply",
            attempt,
            &mut store(&mut db, &partition, &mut error, false, 100_000)
        )
        .is_err()
    );
    assert_eq!(count(&db, "conversation_graph_publications"), 0);
    assert_eq!(count(&db, "effort_awards"), 0);
    assert_eq!(
        host.inspect("run").unwrap().nodes["reply"],
        Disposition::Cancelled
    );
    assert_eq!(
        host.inspect("run").unwrap().artifact,
        executable().artifact()
    );
}

#[test]
fn first_admission_rolls_back_declared_effect_with_native_records() {
    let mut db = db();
    let graph = executable();
    let partition = Partition {
        owner: crate::ai::graph_store::Owner::Conversation("conversation".into()),
        catalog: catalog_id([graph.identity()]).unwrap(),
    };
    let mut error = None;
    assert!(
        DurableEngine::create_with_run(
            [graph.clone()],
            limits(),
            Event::Begin {
                run: "run".into(),
                artifact: graph.identity().into(),
                inputs: inputs("Reply"),
                scope: "captured-authority".into(),
                policy: Default::default(),
            },
            &mut store(&mut db, &partition, &mut error, false, 0),
        )
        .is_err()
    );
    assert!(
        error.is_none(),
        "domain staging succeeded before native record rejection"
    );
    for table in [
        "graph_engines",
        "turn_execution_owners",
        "conversation_graph_effects",
        "conversation_graph_publications",
        "effort_awards",
    ] {
        assert_eq!(count(&db, table), 0);
    }
}

#[tokio::test]
async fn native_adoption_publishes_message_provenance_and_credit_once_and_recovers() {
    let text = "Un cafe\u{301}. 日本語 مرحبًا";
    let (mut db, partition, mut host, attempt) = available(text).await;
    // A settings/context update must not move the captured award attribution.
    db.execute(
        "UPDATE turns SET context=json_set(context,'$.practiceSettings.varietyId','later-variety')",
        [],
    )
    .unwrap();
    let mut error = None;
    host.adopt(
        "run",
        "reply",
        attempt,
        &mut store(&mut db, &partition, &mut error, false, 100_000),
    )
    .unwrap();
    assert!(error.is_none());
    assert_eq!(
        db.query_row(
            "SELECT text FROM messages WHERE role='assistant'",
            [],
            |r| r.get::<_, String>(0)
        )
        .unwrap(),
        text
    );
    assert_eq!(count(&db, "conversation_graph_publications"), 1);
    assert_eq!(count(&db, "effort_awards"), 1);
    assert_eq!(count(&db, "operations"), 0);
    let attribution: (String, String, String) = db
        .query_row(
            "SELECT variety_id,policy,source_id FROM effort_awards",
            [],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
        )
        .unwrap();
    assert_eq!(attribution.0, "captured-variety");
    assert_eq!(attribution.1, "exploration-generation-1");
    assert!(attribution.2.starts_with("graph-effect:"));
    assert_eq!(
        db.query_row("SELECT revision FROM metadata", [], |r| r.get::<_, i64>(0))
            .unwrap(),
        1
    );
    assert_eq!(
        db.query_row("SELECT revision FROM conversations", [], |r| r
            .get::<_, i64>(0))
            .unwrap(),
        2
    );
    assert!(
        host.adopt(
            "run",
            "reply",
            attempt,
            &mut store(&mut db, &partition, &mut error, false, 100_000)
        )
        .is_err()
    );
    let effect: String = db
        .query_row("SELECT id FROM conversation_graph_effects", [], |r| {
            r.get(0)
        })
        .unwrap();
    let tx = db.transaction().unwrap();
    crate::learning::effort::exploration::graph_coach_reply(&tx, &effect).unwrap();
    tx.commit().unwrap();
    assert_eq!(count(&db, "effort_awards"), 1);
    let checkpoint = ReadStore::new(&mut db, partition.clone())
        .unwrap()
        .checkpoint(limits().checkpoint)
        .unwrap()
        .unwrap();
    let recovered = DurableEngine::recover(
        checkpoint,
        [executable()],
        limits(),
        &mut store(&mut db, &partition, &mut error, false, 100_000),
    )
    .unwrap();
    assert_eq!(recovered.outputs("run").unwrap().unwrap()["text"], text);
    db.execute("DELETE FROM conversations", []).unwrap();
    for table in [
        "graph_engines",
        "conversation_graph_effects",
        "conversation_graph_publications",
        "messages",
    ] {
        assert_eq!(count(&db, table), 0);
    }
    assert_eq!(
        count(&db, "effort_awards"),
        1,
        "source deletion retains earned credit"
    );
}

#[tokio::test]
async fn access_publication_and_record_write_failures_rollback_every_effect() {
    for failure in ["access", "award", "records", "archived", "invalidated"] {
        let (mut db, partition, mut host, attempt) = available("Accepted reply").await;
        match failure {
            "award" => db.execute_batch("CREATE TRIGGER reject_award BEFORE INSERT ON effort_awards BEGIN SELECT RAISE(ABORT,'fixture'); END;").unwrap(),
            "archived" => { db.execute("UPDATE conversations SET archived=1",[]).unwrap(); },
            "invalidated" => { db.execute("UPDATE turns SET state='invalidated'",[]).unwrap(); },
            _ => (),
        }
        let stamp = host.stamp().clone();
        let mut error = None;
        assert!(
            host.adopt(
                "run",
                "reply",
                attempt,
                &mut store(
                    &mut db,
                    &partition,
                    &mut error,
                    failure == "access",
                    if failure == "records" { 0 } else { 100_000 }
                )
            )
            .is_err()
        );
        assert_eq!(host.stamp(), &stamp);
        assert_eq!(count(&db, "messages"), 1);
        assert_eq!(count(&db, "conversation_graph_publications"), 0);
        assert_eq!(count(&db, "effort_awards"), 0);
        assert_eq!(
            db.query_row("SELECT revision FROM metadata", [], |r| r.get::<_, i64>(0))
                .unwrap(),
            0
        );
        let checkpoint = ReadStore::new(&mut db, partition.clone())
            .unwrap()
            .checkpoint(limits().checkpoint)
            .unwrap()
            .unwrap();
        assert_eq!(checkpoint.stamp(), &stamp);
        if failure == "award" {
            assert!(
                error.unwrap().diagnostics.is_some(),
                "retain SQLite error metadata"
            );
        }
        match failure {
            "award" => db.execute_batch("DROP TRIGGER reject_award").unwrap(),
            "archived" => {
                db.execute("UPDATE conversations SET archived=0", [])
                    .unwrap();
            }
            "invalidated" => {
                db.execute("UPDATE turns SET state='pending'", []).unwrap();
            }
            _ => (),
        }
        let mut error = None;
        host.adopt(
            "run",
            "reply",
            attempt,
            &mut store(&mut db, &partition, &mut error, false, 100_000),
        )
        .unwrap();
        assert_eq!(count(&db, "effort_awards"), 1);
    }
}

#[tokio::test]
async fn invalid_reply_retains_validation_diagnostics_without_adoption() {
    let (mut db, partition, mut host, attempt) =
        available("assistant: hello\nuser: another turn").await;
    let mut error = None;
    assert!(
        host.adopt(
            "run",
            "reply",
            attempt,
            &mut store(&mut db, &partition, &mut error, false, 100_000)
        )
        .is_err()
    );
    assert_eq!(
        error.unwrap().diagnostics.unwrap()["reason"],
        "role_labeled_transcript"
    );
    assert_eq!(count(&db, "conversation_graph_publications"), 0);
    assert_eq!(count(&db, "effort_awards"), 0);
    assert_eq!(
        host.inspect("run").unwrap().attempts["reply"][0].state,
        AttemptState::Available
    );
}

#[tokio::test]
async fn publication_constraints_preserve_identity_and_exact_u64s() {
    let (db, _, _, _) = available("Reply").await;
    let effect: String = db
        .query_row("SELECT id FROM conversation_graph_effects", [], |r| {
            r.get(0)
        })
        .unwrap();
    assert!(
        db.execute(
            "UPDATE conversation_graph_effects SET variety_id='other'",
            []
        )
        .is_err()
    );
    assert!(crate::learning::effort::exploration::graph_coach_reply(&db, &effect).is_err());
    db.execute("INSERT INTO messages(id,conversation_id,turn_id,sequence,role,text) VALUES('accepted','conversation','turn',2,'assistant','Reply')",[]).unwrap();
    for invalid in [
        "0",
        "01",
        "-1",
        "1.0",
        "1e3",
        "18446744073709551616",
        "",
        "1\0extra",
    ] {
        assert!(
            db.execute(
                "INSERT INTO conversation_graph_publications VALUES(?1,?2,'1','accepted')",
                params![effect, invalid]
            )
            .is_err()
        );
        assert!(
            db.execute(
                "INSERT INTO conversation_graph_publications VALUES(?1,'1',?2,'accepted')",
                params![effect, invalid]
            )
            .is_err()
        );
    }
    assert!(
        db.execute(
            "INSERT INTO conversation_graph_publications VALUES(?1,'1','1','user')",
            [&effect]
        )
        .is_err()
    );
    db.execute("INSERT INTO conversation_graph_publications VALUES(?1,'18446744073709551615','9007199254740993','accepted')",[&effect]).unwrap();
    let ids: (String, String) = db
        .query_row(
            "SELECT attempt_id,execution_id FROM conversation_graph_publications",
            [],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .unwrap();
    assert_eq!(ids, (u64::MAX.to_string(), "9007199254740993".into()));
    assert!(
        db.execute(
            "UPDATE conversation_graph_publications SET attempt_id='2'",
            []
        )
        .is_err()
    );
    assert!(
        db.execute(
            "INSERT INTO conversation_graph_publications VALUES(?1,'2','2','accepted')",
            [&effect]
        )
        .is_err()
    );
}
