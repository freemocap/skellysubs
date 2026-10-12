//! Durable presentation receipts; never another points or XP ledger.
use crate::learning::learner::{progression, skill_levels::SkillLevelSummary};
use crate::model::{AppError, ErrorCode, Result};
use crate::storage::store::Store;
use rusqlite::{Connection, params};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::sync::Arc;
use ts_rs::TS;

pub const CLAIM_LIMIT: usize = 100;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
pub enum SkillLevelEventKind {
    SkillLevel,
    LanguageLevel,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct SkillLevelEvent {
    pub id: String,
    pub sequence: u32,
    pub kind: SkillLevelEventKind,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    #[ts(optional)]
    pub skill_id: Option<String>,
    pub from_level: u32,
    pub to_level: u32,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    #[ts(optional)]
    pub source_attempt_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    #[ts(optional)]
    pub chat_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    #[ts(optional)]
    pub message_id: Option<i32>,
}

pub(crate) struct Source<'a> {
    pub attempt: &'a str,
    pub chat: &'a str,
    pub message: i32,
}

fn invalid(message: &str) -> AppError {
    AppError::new(ErrorCode::Validation, message)
}

fn identity(snapshot: &Value) -> Result<(&str, &str, SkillLevelSummary)> {
    Ok((
        snapshot["learner_id"]
            .as_str()
            .ok_or_else(|| invalid("Missing level event learner."))?,
        snapshot["target"]
            .as_str()
            .ok_or_else(|| invalid("Missing level event language."))?,
        serde_json::from_value(snapshot["profile"]["levels"].clone())?,
    ))
}

/// Borrow the owning publication transaction; do not commit independently.
pub(crate) fn synchronize(
    tx: &Connection,
    snapshot: &Value,
    source: Option<Source<'_>>,
) -> Result<()> {
    if tx.is_autocommit() {
        return Err(AppError::new(
            ErrorCode::Conflict,
            "Skill-level publication requires a transaction.",
        ));
    }
    let (learner, target, levels) = identity(snapshot)?;
    let maximum = levels
        .skills
        .iter()
        .map(|skill| skill.level)
        .max()
        .unwrap_or(0);
    for level in 1..=maximum {
        for skill in levels.skills.iter().filter(|skill| skill.level >= level) {
            insert(
                tx,
                learner,
                target,
                &levels.policy_id,
                "skill_level",
                &skill.skill_id,
                level,
                source.as_ref(),
            )?;
        }
        if levels.level >= level {
            insert(
                tx,
                learner,
                target,
                &levels.policy_id,
                "language_level",
                "",
                level,
                source.as_ref(),
            )?;
        }
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn insert(
    tx: &Connection,
    learner: &str,
    target: &str,
    policy: &str,
    kind: &str,
    skill: &str,
    level: u32,
    source: Option<&Source<'_>>,
) -> Result<()> {
    // No foreign key to the source: deleting a conversation must not re-arm a milestone.
    // Avoid ignored inserts incrementing AUTOINCREMENT on every snapshot refresh.
    tx.execute("INSERT INTO skill_level_events(id,learner_id,language_id,policy_id,kind,skill_id,from_level,to_level,source_attempt_id,chat_id,message_id)
        SELECT ?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11 WHERE NOT EXISTS
        (SELECT 1 FROM skill_level_events WHERE learner_id=?2 AND language_id=?3 AND policy_id=?4 AND kind=?5 AND skill_id=?6 AND to_level=?8)",
        params![uuid::Uuid::new_v4().to_string(),learner,target,policy,kind,skill,level-1,level,
            source.map(|s|s.attempt),source.map(|s|s.chat),source.map(|s|s.message)])?;
    Ok(())
}

/// Only currently eligible receipts are offered; historical receipts stay intact.
pub(crate) fn pending(db: &Connection, snapshot: &Value) -> Result<Vec<SkillLevelEvent>> {
    let (learner, target, levels) = identity(snapshot)?;
    let mut statement = db.prepare("SELECT id,sequence,kind,skill_id,from_level,to_level,source_attempt_id,chat_id,message_id
        FROM skill_level_events WHERE learner_id=?1 AND language_id=?2 AND policy_id=?3 AND claimed=0 ORDER BY sequence")?;
    let rows = statement.query_map(params![learner, target, levels.policy_id], |r| {
        let kind: String = r.get(2)?;
        Ok(SkillLevelEvent {
            id: r.get(0)?,
            sequence: r.get(1)?,
            kind: if kind == "skill_level" {
                SkillLevelEventKind::SkillLevel
            } else {
                SkillLevelEventKind::LanguageLevel
            },
            skill_id: if kind == "skill_level" {
                Some(r.get(3)?)
            } else {
                None
            },
            from_level: r.get(4)?,
            to_level: r.get(5)?,
            source_attempt_id: r.get(6)?,
            chat_id: r.get(7)?,
            message_id: r.get(8)?,
        })
    })?;
    let mut events = Vec::new();
    for row in rows {
        let event = row?;
        let level = if let Some(skill) = &event.skill_id {
            levels
                .skills
                .iter()
                .find(|s| &s.skill_id == skill)
                .map(|s| s.level)
                .unwrap_or(0)
        } else {
            levels.level
        };
        if event.to_level <= level {
            events.push(event);
            if events.len() == CLAIM_LIMIT {
                break;
            }
        }
    }
    Ok(events)
}

pub(crate) fn initialize(store: &mut Store, target: &str) -> Result<Vec<SkillLevelEvent>> {
    let tx = store.connection.transaction()?;
    let snapshot = progression::snapshot_db(&tx, &store.config, &store.session_id, target)?;
    synchronize(&tx, &snapshot, None)?;
    let events = pending(&tx, &snapshot)?;
    tx.commit()?;
    Ok(events)
}

/// At-most-once claim. A crash after commit can skip presentation, never reaward XP.
pub(crate) fn claim(
    store: &mut Store,
    target: &str,
    ids: &[String],
) -> Result<Vec<SkillLevelEvent>> {
    if ids.len() > CLAIM_LIMIT
        || ids.iter().collect::<std::collections::BTreeSet<_>>().len() != ids.len()
    {
        return Err(invalid(
            "Level claims must contain at most 100 distinct IDs.",
        ));
    }
    let tx = store.connection.transaction()?;
    let snapshot = progression::snapshot_db(&tx, &store.config, &store.session_id, target)?;
    let (learner, _, levels) = identity(&snapshot)?;
    let mut unclaimed = Vec::new();
    for id in ids {
        use rusqlite::OptionalExtension;
        let claimed: Option<bool> = tx.query_row("SELECT claimed FROM skill_level_events WHERE id=?1 AND learner_id=?2 AND language_id=?3 AND policy_id=?4",
            params![id,learner,target,levels.policy_id], |r|r.get(0)).optional()?;
        match claimed {
            Some(false) => unclaimed.push(id.as_str()),
            Some(true) => {}
            None => {
                return Err(invalid(
                    "Level claim does not belong to this learner, language and policy.",
                ));
            }
        }
    }
    let pending = pending(&tx, &snapshot)?;
    if !unclaimed.iter().copied().eq(pending
        .iter()
        .take(unclaimed.len())
        .map(|event| event.id.as_str()))
    {
        return Err(AppError::new(
            ErrorCode::Conflict,
            "Level events changed. Refresh before claiming the next ordered batch.",
        ));
    }
    let accepted: Vec<_> = pending.into_iter().take(unclaimed.len()).collect();
    for event in &accepted {
        tx.execute(
            "UPDATE skill_level_events SET claimed=1 WHERE id=?1 AND claimed=0",
            [&event.id],
        )?;
    }
    tx.commit()?;
    Ok(accepted)
}

#[tauri::command]
pub(crate) fn initialize_skill_level_events(
    state: tauri::State<'_, Arc<crate::application::Application>>,
    target: String,
) -> Result<Vec<SkillLevelEvent>> {
    let mut store = state.lock()?;
    initialize(&mut store, &target)
}

#[tauri::command]
pub(crate) fn claim_skill_level_events(
    state: tauri::State<'_, Arc<crate::application::Application>>,
    target: String,
    ids: Vec<String>,
) -> Result<Vec<SkillLevelEvent>> {
    let mut store = state.lock()?;
    claim(&mut store, &target, &ids)
}

#[cfg(test)]
#[path = "skill_level_events_tests.rs"]
mod tests;
