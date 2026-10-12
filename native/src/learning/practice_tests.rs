use super::*;
use rusqlite::Connection;
fn database() -> Connection {
    let db = Connection::open_in_memory().unwrap();
    db.execute_batch("CREATE TABLE turns(id TEXT PRIMARY KEY,context TEXT NOT NULL,replaces_turn_id TEXT,conversation_id TEXT,state TEXT); CREATE TABLE messages(turn_id TEXT,role TEXT,text TEXT); CREATE TABLE conversation_graph_assessments(id TEXT PRIMARY KEY,turn_id TEXT,kind TEXT);").unwrap();
    db
}
fn add(db: &Connection, id: &str, parent: Option<&str>, text: &str) {
    db.execute("INSERT INTO turns VALUES(?1,'{\"practiceSettings\":{\"varietyId\":\"levantine\"}}',?2,'chat','pending')",params![id,parent]).unwrap();
    db.execute(
        "INSERT INTO messages VALUES(?1,'user',?2)",
        params![id, text],
    )
    .unwrap();
    db.execute(
        "INSERT INTO conversation_graph_assessments VALUES(?1,?1,'skill_assessment')",
        [id],
    )
    .unwrap();
}
fn present() -> BTreeMap<String, Presence> {
    BTreeMap::from([
        ("past".into(), Presence::Direct),
        ("possession".into(), Presence::Contextual),
    ])
}
fn save(db: &mut Connection, id: &str) -> Observation {
    let tx = db.transaction().unwrap();
    let p = present();
    let result = publish_graph(&tx, id, id, p.clone(), &p.keys().cloned().collect()).unwrap();
    tx.commit().unwrap();
    result
}
#[test]
fn persisted_retry_counts_survive_reload_and_duplicate_delivery() {
    let mut db = database();
    add(&db, "a", None, "مبارح");
    let a = save(&mut db, "a");
    assert!(a.credits.iter().all(|c| c.experience == 1 && c.effort == 0));
    assert_eq!(a, save(&mut db, "a"));
    add(&db, "b", Some("a"), "مبارح رحت");
    let b = save(&mut db, "b");
    assert!(b.credits.iter().all(|c| c.effort == 1 && c.experience == 0));
    add(&db, "c", Some("b"), "مبارح رحت");
    assert!(save(&mut db, "c").credits.is_empty());
    add(&db, "d", None, "مبارح رحت");
    assert!(save(&mut db, "d").credits.iter().all(|c| c.experience == 1));
}
#[test]
fn absent_and_unclear_do_not_earn_credit_new_skill_does() {
    let p = BTreeMap::from([
        ("known".into(), Presence::Contextual),
        ("new".into(), Presence::Direct),
        ("absent".into(), Presence::Absent),
        ("unclear".into(), Presence::Unclear),
    ]);
    let r = credits(&p, &BTreeSet::from(["known".into()]), true);
    assert_eq!(r.len(), 2);
    assert_eq!(r[0].effort, 1);
    assert_eq!(r[1].experience, 1);
}
#[test]
fn invalidated_or_superseded_attempts_cannot_publish_graph() {
    let mut db = database();
    add(&db, "a", None, "one");
    add(&db, "b", Some("a"), "two");
    let tx = db.transaction().unwrap();
    let p = present();
    let ids = p.keys().cloned().collect();
    assert!(publish_graph(&tx, "a", "a", p.clone(), &ids).is_err());
    tx.execute("UPDATE turns SET state='invalidated' WHERE id='b'", [])
        .unwrap();
    assert!(publish_graph(&tx, "b", "b", p, &ids).is_err());
}
#[test]
fn transaction_rollback_does_not_leave_credit() {
    let mut db = database();
    add(&db, "a", None, "one");
    {
        let tx = db.transaction().unwrap();
        let p = present();
        publish_graph(&tx, "a", "a", p.clone(), &p.keys().cloned().collect()).unwrap();
    }
    let n: bool = db
        .query_row(
            "SELECT json_type(context,'$.practiceObservation') IS NULL FROM turns WHERE id='a'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert!(n);
    assert_eq!(save(&mut db, "a").credits.len(), 2);
}
#[test]
fn rejects_conflicting_duplicate_and_incomplete_catalog() {
    let mut db = database();
    add(&db, "a", None, "one");
    save(&mut db, "a");
    let tx = db.transaction().unwrap();
    let mut p = present();
    let ids = p.keys().cloned().collect();
    p.insert("past".into(), Presence::Absent);
    assert!(publish_graph(&tx, "a", "a", p.clone(), &ids).is_err());
    p.remove("past");
    assert!(publish_graph(&tx, "a", "a", p, &ids).is_err());
}
#[test]
fn rejects_cross_conversation_revision_and_preserves_other_context() {
    let mut db = database();
    add(&db, "a", None, "one");
    save(&mut db, "a");
    add(&db, "b", Some("a"), "two");
    db.execute("UPDATE turns SET conversation_id='other' WHERE id='b'", [])
        .unwrap();
    {
        let tx = db.transaction().unwrap();
        let p = present();
        assert!(publish_graph(&tx, "b", "b", p.clone(), &p.keys().cloned().collect()).is_err());
    }
    db.execute("UPDATE turns SET conversation_id='chat',context=json_set(context,'$.messageRatings',json('{\"grammar\":7}')) WHERE id='b'",[]).unwrap();
    save(&mut db, "b");
    let score: i32 = db
        .query_row(
            "SELECT json_extract(context,'$.messageRatings.grammar') FROM turns WHERE id='b'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(score, 7);
}

#[test]
fn native_receipts_preserve_revision_credit_and_reject_conflicting_delivery() {
    let mut db = database();
    for (id, parent, text, expected) in [
        ("a", None, "café 日本語", (1, 0)),
        ("b", Some("a"), "café 日本語!", (0, 1)),
        ("c", Some("b"), "café 日本語!", (0, 0)),
    ] {
        add(&db, id, parent, text);
        let receipt = format!("native:{id}");
        let tx = db.transaction().unwrap();
        tx.execute(
            "INSERT INTO conversation_graph_assessments VALUES(?1,?2,'skill_assessment')",
            params![receipt, id],
        )
        .unwrap();
        let presence = present();
        let ids = presence.keys().cloned().collect();
        let observation = publish_graph(&tx, id, &receipt, presence.clone(), &ids).unwrap();
        if expected == (0, 0) {
            assert!(observation.credits.is_empty());
        } else {
            assert_eq!(observation.credits.len(), 2);
            assert!(
                observation
                    .credits
                    .iter()
                    .all(|c| (c.experience, c.effort) == expected)
            );
        }
        assert_eq!(
            publish_graph(&tx, id, &receipt, presence.clone(), &ids).unwrap(),
            observation
        );
        assert!(publish_graph(&tx, id, "another-attempt", presence, &ids).is_err());
        tx.commit().unwrap();
    }
}
