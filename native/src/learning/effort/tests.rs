use super::*;

#[test]
fn inactive_awards_are_preserved_but_not_counted_reported_or_claimed() {
    let mut db = database();
    db.execute("INSERT INTO effort_awards(id,dimension,source_id,language_id,variety_id,policy,claimed) VALUES('saved','no_issues_flagged','message','spanish','standard','saved-policy',0)", []).unwrap();
    let before: (String, String, i64) = db
        .query_row(
            "SELECT source_id,policy,claimed FROM effort_awards WHERE id='saved'",
            [],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
        )
        .unwrap();
    assert!(read(&db, "spanish").unwrap().recent.is_empty());
    let report = report::read(&db, "spanish", None, None).unwrap();
    assert!(report.entries.is_empty());
    assert!(report.activity.is_empty());
    assert!(
        claim(&mut db, "spanish", &["saved".into()])
            .unwrap()
            .is_empty()
    );
    let after = db
        .query_row(
            "SELECT source_id,policy,claimed FROM effort_awards WHERE id='saved'",
            [],
            |r| {
                Ok((
                    r.get::<_, String>(0)?,
                    r.get::<_, String>(1)?,
                    r.get::<_, i64>(2)?,
                ))
            },
        )
        .unwrap();
    assert_eq!(after, before);
}

#[test]
fn coaching_feedback_cannot_create_effort_awards() {
    let db = database();
    message(&db, "source", "coach_feedback").unwrap();
    assert_eq!(
        db.query_row("SELECT count(*) FROM effort_awards", [], |r| r
            .get::<_, i64>(0))
            .unwrap(),
        0
    );
}
fn database() -> Connection {
    let db = Connection::open_in_memory().unwrap();
    db.execute_batch(include_str!("../../storage/schemas/schema.sql"))
        .unwrap();
    db.execute_batch(include_str!("../../storage/schemas/graph_runtime.sql"))
        .unwrap();
    db.execute_batch(include_str!(
        "../../storage/schemas/turn_execution_owners.sql"
    ))
    .unwrap();
    db
}
#[test]
fn lifetime_awards_are_idempotent_scoped_and_separate_from_presentation() {
    let mut db = database();
    for _ in 0..2 {
        award(
            &db,
            EffortDimension::RevisionsSent,
            "revision",
            "spanish",
            "standard",
            Some("deleted-chat"),
        )
        .unwrap();
    }
    award(
        &db,
        EffortDimension::PartnerUnderstood,
        "revision",
        "spanish",
        "standard",
        Some("deleted-chat"),
    )
    .unwrap();
    award(
        &db,
        EffortDimension::PracticeAttempts,
        "take",
        "french",
        "standard",
        None,
    )
    .unwrap();
    let progress = read(&db, "spanish").unwrap();
    assert_eq!(
        (
            progress.revisions_sent,
            progress.partner_understood,
            progress.practice_attempts
        ),
        (1, 1, 0)
    );
    let ids: Vec<_> = progress.recent.iter().map(|e| e.id.clone()).collect();
    assert!(claim(&mut db, "french", &ids).unwrap().is_empty());
    assert_eq!(claim(&mut db, "spanish", &ids).unwrap().len(), 2);
    assert!(claim(&mut db, "spanish", &ids).unwrap().is_empty());
    assert_eq!(read(&db, "spanish").unwrap().revisions_sent, 1);
    // No source foreign keys: removal of source history cannot cascade into awards.
    assert_eq!(
        db.query_row(
            "SELECT count(*) FROM pragma_foreign_key_list('effort_awards')",
            [],
            |r| r.get::<_, i64>(0)
        )
        .unwrap(),
        0
    );
    db.execute("DELETE FROM conversations", []).unwrap();
    db.execute("DELETE FROM drill_items", []).unwrap();
    assert_eq!(read(&db, "spanish").unwrap().recent.len(), 2);
}
#[test]
fn source_rollback_rolls_back_credit() {
    let mut db = database();
    {
        let tx = db.transaction().unwrap();
        award(
            &tx,
            EffortDimension::PracticeAttempts,
            "take",
            "spanish",
            "standard",
            None,
        )
        .unwrap();
    }
    assert_eq!(read(&db, "spanish").unwrap().practice_attempts, 0);
}
#[test]
fn awards_and_claims_survive_reopening() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("progress.sqlite3");
    let mut db = Connection::open(&path).unwrap();
    db.execute_batch(include_str!("../../storage/schemas/schema.sql"))
        .unwrap();
    award(
        &db,
        EffortDimension::RevisionsSent,
        "message",
        "spanish",
        "standard",
        None,
    )
    .unwrap();
    let id = read(&db, "spanish").unwrap().recent[0].id.clone();
    claim(&mut db, "spanish", std::slice::from_ref(&id)).unwrap();
    drop(db);
    let mut db = Connection::open(&path).unwrap();
    award(
        &db,
        EffortDimension::RevisionsSent,
        "message",
        "spanish",
        "standard",
        None,
    )
    .unwrap();
    assert_eq!(read(&db, "spanish").unwrap().revisions_sent, 1);
    assert!(claim(&mut db, "spanish", &[id]).unwrap().is_empty());
}

#[test]
fn recorded_practice_counts_once_even_after_attempt_and_item_cleanup() {
    let directory = tempfile::tempdir().unwrap();
    let mut store =
        crate::storage::store::Store::open(&directory.path().join("practice.sqlite3")).unwrap();
    let item = store
        .create_drill_item(crate::drill::DrillItemInput {
            text: "Quisiera un café".into(),
            language: "spanish".into(),
            variety: None,
            explanation: "english".into(),
            explanation_variety: None,
        })
        .unwrap();
    let tx = store.connection.transaction().unwrap();
    tx.execute("INSERT INTO transcription_attempts(id,drill_item_id,route,model,profile_revision,state) VALUES('recording',?1,'custom','fixture',1,'succeeded')",[&item.id]).unwrap();
    let reliability = crate::drill::reliability::DrillReliability {
        policy: 1,
        accepted: false,
        confidence: None,
        minimum_confidence: 0.6,
        speech_seconds: 1.0,
        no_speech_probability: None,
        source: "unavailable".into(),
        reason: "confidence_unavailable".into(),
    };
    for _ in 0..2 {
        crate::drill::stage_attempt_with_reliability(
            &tx,
            &item.id,
            Some("recording"),
            "quisiera cafe",
            None,
            Some(reliability.clone()),
        )
        .unwrap();
        tx.execute("DELETE FROM drill_attempts", []).unwrap();
    }
    tx.execute("DELETE FROM drill_items WHERE id=?1", [&item.id])
        .unwrap();
    tx.commit().unwrap();
    assert_eq!(
        read(&store.connection, "spanish")
            .unwrap()
            .practice_attempts,
        1
    );
}

#[test]
fn report_totals_cover_all_history_and_pages_do_not_duplicate_new_arrivals() {
    let db = database();
    for number in 0..105 {
        award(
            &db,
            EffortDimension::PracticeAttempts,
            &format!("take-{number}"),
            "spanish",
            "standard",
            None,
        )
        .unwrap();
    }
    db.execute(
        "UPDATE effort_awards SET created_at='2000-01-01T12:00:00Z' WHERE source_id='take-0'",
        [],
    )
    .unwrap();
    let first = report::read(&db, "spanish", None, None).unwrap();
    assert_eq!(first.activity[0].total, 105);
    assert_eq!(first.activity[0].last_seven_days, 104);
    assert_eq!(first.activity[0].active_days, 2);
    assert_eq!(first.entries.len(), 50);
    assert!(
        first
            .entries
            .iter()
            .all(|entry| entry.source_text.is_none())
    );
    award(
        &db,
        EffortDimension::PracticeAttempts,
        "new-arrival",
        "spanish",
        "standard",
        None,
    )
    .unwrap();
    let second = report::read(&db, "spanish", None, first.next.as_deref()).unwrap();
    assert_eq!(second.entries.len(), 50);
    assert!(
        second
            .entries
            .iter()
            .all(|entry| first.entries.iter().all(|old| old.id != entry.id))
    );
    let third = report::read(&db, "spanish", None, second.next.as_deref()).unwrap();
    assert_eq!(third.entries.len(), 5);
    assert!(third.next.is_none());
    assert!(report::read(&db, "french", None, first.next.as_deref()).is_err());
    assert!(
        report::read(&db, "spanish", Some(EffortDimension::RevisionsSent), None)
            .unwrap()
            .entries
            .is_empty()
    );
    assert_eq!(
        read(&db, "spanish")
            .unwrap()
            .recent
            .iter()
            .filter(|award| award.claimed)
            .count(),
        0
    );
}
#[test]
fn conversation_scope_counts_only_that_conversation() {
    let db = database();
    for (dimension, source, conversation) in [
        (EffortDimension::PartnerUnderstood, "a1", Some("chat-a")),
        (EffortDimension::RevisionsSent, "a1", Some("chat-a")),
        (EffortDimension::PartnerUnderstood, "b1", Some("chat-b")),
        (EffortDimension::PracticeAttempts, "take", None),
    ] {
        award(&db, dimension, source, "spanish", "standard", conversation).unwrap();
    }
    let chat = read_scoped(&db, "spanish", Some("chat-a")).unwrap();
    assert_eq!(
        (
            chat.partner_understood,
            chat.revisions_sent,
            chat.practice_attempts
        ),
        (1, 1, 0)
    );
    assert_eq!(chat.recent.len(), 2);
    let all = read(&db, "spanish").unwrap();
    assert_eq!((all.partner_understood, all.practice_attempts), (2, 1));
}
#[test]
fn practice_counted_follows_the_receipt_award() {
    let db = database();
    assert!(!practice_counted(&db, Some("receipt")).unwrap());
    assert!(!practice_counted(&db, None).unwrap());
    award(
        &db,
        EffortDimension::PracticeAttempts,
        "receipt",
        "spanish",
        "standard",
        None,
    )
    .unwrap();
    assert!(practice_counted(&db, Some("receipt")).unwrap());
    assert!(!practice_counted(&db, Some("other")).unwrap());
}
