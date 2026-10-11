//! Compact totals over the durable reward ledger; reading does not publish credit.
use super::progression::LanguageTotals;
use crate::{model::*, storage::store::Store};
use rusqlite::OptionalExtension;
use serde_json::Value;
use std::collections::BTreeSet;

pub(super) fn read(store: &Store) -> Result<Vec<LanguageTotals>> {
    // Keep one consistent read transaction across the projection. Autocommit
    // would repeatedly acquire/release SQLite file locks for its small queries.
    let transaction = store
        .connection
        .is_autocommit()
        .then(|| store.connection.unchecked_transaction())
        .transpose()?;
    let hash = crate::learning::coaching::construct_hash(&store.config);
    // Scan assessment contexts once. Filtering separately per catalog language
    // repeatedly parses the same persisted JSON through SQLite's join plan.
    let mut contexts = std::collections::BTreeMap::<String, Vec<(String, Value)>>::new();
    let mut statement = store.connection.prepare("SELECT c.language_id,t.id,t.context FROM turns t JOIN conversations c ON c.id=t.conversation_id JOIN messages m ON m.turn_id=t.id AND m.role='user' WHERE json_type(t.context,'$.skillAssessment')='object'")?;
    let records = statement.query_map([], |r| {
        Ok((
            r.get::<_, String>(0)?,
            r.get::<_, String>(1)?,
            r.get::<_, String>(2)?,
        ))
    })?;
    for record in records {
        let (language, turn, raw) = record?;
        contexts
            .entry(language)
            .or_default()
            .push((turn, serde_json::from_str(&raw)?));
    }
    let catalog = store.config.shared_practice_catalog();
    let mut rows = vec![];
    for language in store.config.language_projection() {
        let excluded: Option<String> = store
            .connection
            .query_row(
                "SELECT excluded FROM skill_choices WHERE language_id=?1",
                [&language.id],
                |r| r.get(0),
            )
            .optional()?;
        let excluded: BTreeSet<String> = serde_json::from_str(excluded.as_deref().unwrap_or("[]"))?;
        let mut eligible = vec![];
        for (turn, context) in contexts.get(&language.id).into_iter().flatten() {
            let credits = crate::learning::rewards::credits(context)?;
            let attempt = context["skillAssessmentAttempt"].as_str().unwrap_or(turn);
            if context["constructRegistryHash"] != hash || excluded.contains(attempt) {
                continue;
            }
            eligible.extend(credits);
        }
        // Preserve catalog identity and duplicate-credit validation through its owner.
        super::skill_levels::project(&catalog, &eligible)?;
        let mut xp = 0_u32;
        for credit in eligible {
            let amount = credit["xp"]
                .as_u64()
                .and_then(|n| u32::try_from(n).ok())
                .ok_or_else(invalid)?;
            xp = xp.checked_add(amount).ok_or_else(invalid)?;
        }
        let count: u32 = store.connection.query_row(
            "SELECT count(*) FROM conversations WHERE language_id=?1",
            [&language.id],
            |r| r.get(0),
        )?;
        let effort = crate::learning::effort::read(&store.connection, &language.id)?;
        rows.push(LanguageTotals {
            target: language.id,
            name: language.name,
            native_name: language.native_name,
            language_tag: language.language_tag,
            xp,
            conversations: count,
            partner_understood: effort.partner_understood,
            revisions_sent: effort.revisions_sent,
            practice_attempts: effort.practice_attempts,
            explorations: effort.explorations,
            bot: effort.bot,
        });
    }
    if let Some(transaction) = transaction {
        transaction.commit()?;
    }
    Ok(rows)
}

fn invalid() -> AppError {
    AppError::new(ErrorCode::Validation, "Language totals have invalid XP.")
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    #[test]
    fn totals_match_evidence_for_eligible_excluded_and_different_registry_credit() {
        let dir = tempfile::tempdir().unwrap();
        let mut store = Store::open(&dir.path().join("totals.sqlite3")).unwrap();
        store.prepare_chat().unwrap();
        let chat = store.snapshot().unwrap().conversations[0].id.clone();
        let hash = crate::learning::coaching::construct_hash(&store.config);
        for (index, (id, registry, skill)) in [
            ("included", hash.as_str(), "information_exchange"),
            ("excluded", hash.as_str(), "information_exchange"),
            ("different", "different", "information_exchange"),
        ]
        .into_iter()
        .enumerate()
        {
            let event = json!({"id":id,"attemptId":id,"constructId":skill,"kind":"experience","tier":1,"xp":7,"experience":1,"effort":0,"quote":"𠮷 café","support":"not_weighted","difficulty":"not_weighted","novelty":"not_weighted","policyHash":"experience-effort-1","atSecs":1,"claimed":false});
            let context = json!({"constructRegistryHash":registry,"skillAssessmentAttempt":id,"skillAssessment":{"answers":{}},"rewardEvents":[event],"gamePolicy":{}});
            store.connection.execute("INSERT INTO turns(id,conversation_id,state,paused,profile_revision,credential_id,route,model,context) VALUES(?1,?2,'succeeded',0,1,'fixture','custom','fixture',?3)",rusqlite::params![id,chat,context.to_string()]).unwrap();
            store.connection.execute("INSERT INTO messages(id,conversation_id,turn_id,sequence,role,text,created_at) VALUES(?1,?2,?1,?3,'user','𠮷 café','2020-01-01T00:00:00Z')",rusqlite::params![id,chat,index as i32+1]).unwrap();
        }
        store.connection.execute("INSERT INTO skill_choices(language_id,revision,focus,excluded) VALUES('spanish',1,NULL,'[\"excluded\"]')",[]).unwrap();
        let before = store.connection.total_changes();
        for row in read(&store).unwrap() {
            let evidence = super::super::progression::snapshot_db(
                &store.connection,
                &store.config,
                &store.session_id,
                &row.target,
            )
            .unwrap();
            assert_eq!(
                u64::from(row.xp),
                evidence["profile"]["xp"].as_u64().unwrap()
            );
            assert_eq!(
                u64::from(row.conversations),
                evidence["conversation_count"].as_u64().unwrap()
            );
            if row.target == "spanish" {
                assert_eq!(row.xp, 7);
            }
        }
        assert_eq!(store.connection.total_changes(), before);
        let transaction = store.connection.unchecked_transaction().unwrap();
        store.connection.execute("INSERT INTO effort_awards(id,dimension,source_id,language_id,variety_id,conversation_id,policy) VALUES('nested','practice_attempts','nested','spanish','spanish-spain',NULL,'test')",[]).unwrap();
        assert_eq!(
            read(&store)
                .unwrap()
                .iter()
                .find(|r| r.target == "spanish")
                .unwrap()
                .practice_attempts,
            1
        );
        transaction.rollback().unwrap();
        assert_eq!(
            read(&store)
                .unwrap()
                .iter()
                .find(|r| r.target == "spanish")
                .unwrap()
                .practice_attempts,
            0
        );
        store.connection.execute("UPDATE turns SET context=json_set(context,'$.rewardEvents[0].constructId','unknown') WHERE id='included'",[]).unwrap();
        assert!(read(&store).is_err());
        store.connection.execute("UPDATE turns SET context=json_set(context,'$.rewardEvents[0].constructId','information_exchange') WHERE id='included'",[]).unwrap();
        store.connection.execute("UPDATE turns SET context=json_set(context,'$.rewardEvents[1]',json_extract(context,'$.rewardEvents[0]')) WHERE id='included'",[]).unwrap();
        assert!(read(&store).is_err());
        store.connection.execute("UPDATE turns SET context=json_set(context,'$.rewardEvents',json('[{}]')) WHERE id='included'",[]).unwrap();
        assert!(read(&store).is_err());
    }
}
