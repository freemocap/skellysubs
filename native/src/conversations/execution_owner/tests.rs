use super::*;
use rusqlite::params;

fn db() -> Connection {
    let db = Connection::open_in_memory().unwrap();
    db.execute_batch("PRAGMA foreign_keys=ON; CREATE TABLE conversations(id TEXT PRIMARY KEY); CREATE TABLE turns(id TEXT PRIMARY KEY,conversation_id TEXT REFERENCES conversations(id) ON DELETE CASCADE); INSERT INTO conversations VALUES('one'),('two'); INSERT INTO turns VALUES('turn','one');").unwrap();
    db.execute_batch(include_str!("../../storage/schemas/graph_runtime.sql"))
        .unwrap();
    db.execute_batch(include_str!(
        "../../storage/schemas/turn_execution_owners.sql"
    ))
    .unwrap();
    db
}
fn engine(db: &Connection, conversation: &str) -> rusqlite::Result<usize> {
    db.execute(
        "INSERT INTO graph_engines VALUES('engine',?1,?2,'{}',x'00')",
        params![conversation, "a".repeat(64)],
    )
}
fn graph_owner(db: &Connection) -> rusqlite::Result<usize> {
    db.execute("INSERT INTO turn_execution_owners VALUES('turn','graph','coach','engine','run','artifact')",[])
}

#[test]
fn first_engine_reference_is_deferred_but_cross_conversation_ownership_is_rejected() {
    for engine_first in [true, false] {
        for conversation in ["one", "two"] {
            let mut db = db();
            let tx = db.transaction().unwrap();
            let result = if engine_first {
                engine(&tx, conversation).unwrap();
                graph_owner(&tx)
            } else {
                graph_owner(&tx).unwrap();
                engine(&tx, conversation)
            };
            if conversation == "one" {
                result.unwrap();
                tx.commit().unwrap();
                validate_complete(&db).unwrap();
            } else {
                assert!(result.is_err());
                tx.rollback().unwrap();
                assert_eq!(
                    db.query_row("SELECT count(*) FROM turn_execution_owners", [], |r| r
                        .get::<_, i64>(0))
                        .unwrap(),
                    0
                );
            }
        }
    }
    let mut db = db();
    let tx = db.transaction().unwrap();
    graph_owner(&tx).unwrap();
    assert!(tx.commit().is_err(), "an absent engine cannot be committed");
    assert!(validate_complete(&db).is_err());
}

#[test]
fn ownership_is_complete_immutable_and_deleted_with_its_turn() {
    let db = db();
    assert!(validate_complete(&db).is_err());
    engine(&db, "one").unwrap();
    graph_owner(&db).unwrap();
    validate_complete(&db).unwrap();
    assert!(graph_owner(&db).is_err());
    assert!(
        db.execute("UPDATE turns SET conversation_id='two' WHERE id='turn'", [])
            .is_err()
    );
    assert!(
        db.execute(
            "UPDATE turn_execution_owners SET channel='persona_reply'",
            []
        )
        .is_err()
    );
    db.execute("DELETE FROM turns WHERE id='turn'", []).unwrap();
    assert_eq!(
        db.query_row("SELECT count(*) FROM turn_execution_owners", [], |r| r
            .get::<_, i64>(0))
            .unwrap(),
        0
    );
    validate_complete(&db).unwrap();
}

#[test]
fn malformed_executor_identity_cannot_be_stored() {
    let db = db();
    for row in [
        "'invalid','coach','engine',NULL,NULL",
        "'graph','coach',NULL,NULL,NULL",
        "'graph','unknown','engine','run','artifact'",
    ] {
        assert!(
            db.execute(
                &format!("INSERT INTO turn_execution_owners VALUES('turn',{row})"),
                []
            )
            .is_err()
        );
    }
}
