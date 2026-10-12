//! Extend accepted reply effects without changing existing effect/publication IDs.
//! Publications are the only FK children of effects; both tables are copied and
//! restored within the migration chain transaction. Messages and awards stay put.
use super::*;

pub(super) fn apply(db: &Connection) -> Result<()> {
    v60_graph_transport::validate(db)?;
    db.execute_batch(
        "CREATE TEMP TABLE graph_effects_61 AS SELECT * FROM conversation_graph_effects;
         CREATE TEMP TABLE graph_publications_61 AS SELECT * FROM conversation_graph_publications;
         DROP TABLE conversation_graph_publications;
         DROP TABLE conversation_graph_effects;",
    )?;
    db.execute_batch(include_str!("v61_graph_reply_roles.sql"))?;
    // Attribution was captured at admission. Current turn settings may differ;
    // restoring an existing effect must not repeat admission against new settings.
    let owner_trigger: String = db.query_row(
        "SELECT sql FROM sqlite_master WHERE name='conversation_graph_effect_owner'",
        [],
        |r| r.get(0),
    )?;
    db.execute_batch("DROP TRIGGER conversation_graph_effect_owner;")?;
    db.execute_batch(
        "INSERT INTO conversation_graph_effects SELECT * FROM graph_effects_61;
         INSERT INTO conversation_graph_publications SELECT * FROM graph_publications_61;
         DROP TABLE graph_publications_61;
         DROP TABLE graph_effects_61;",
    )?;
    db.execute_batch(&owner_trigger)?;
    Ok(())
}

pub(super) fn validate(db: &Connection) -> Result<()> {
    v58_turn_execution_owners::validate(db)?;
    let reference = Connection::open_in_memory()?;
    reference.execute_batch(include_str!("v61_graph_reply_roles.sql"))?;
    reference.execute_batch(include_str!("v60_graph_transport.sql"))?;
    schema::validate_objects(db, &reference, false)
}
