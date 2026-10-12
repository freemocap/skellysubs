//! Presence-only practice publication for the new skills flow.
//! The caller supplies validated observations inside its completion transaction.
//! Invoked by skill-assessment publication before the owning attempt completes.
use crate::model::{AppError, ErrorCode, Result};
use rusqlite::{Connection, OptionalExtension, params};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

pub const POLICY: &str = "experience-effort-1";
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Presence {
    Absent,
    Contextual,
    Direct,
    Unclear,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Credit {
    pub skill_id: String,
    pub experience: u32,
    pub effort: u32,
    pub xp: u32,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Observation {
    pub policy: String,
    pub turn_id: String,
    pub inference_attempt_id: String,
    pub presence: BTreeMap<String, Presence>,
    pub credits: Vec<Credit>,
}
fn invalid(message: &str) -> AppError {
    AppError::new(ErrorCode::Validation, message)
}

/// Previously encountered means previously credited in this revision chain,
/// not in unrelated messages. Text comparison is exact and preserves Unicode.
pub fn credits(
    present: &BTreeMap<String, Presence>,
    encountered: &BTreeSet<String>,
    changed: bool,
) -> Vec<Credit> {
    if !changed {
        return vec![];
    }
    present
        .iter()
        .filter(|(_, p)| matches!(p, Presence::Direct | Presence::Contextual))
        .map(|(skill_id, _)| {
            let experience = u32::from(!encountered.contains(skill_id));
            Credit {
                skill_id: skill_id.clone(),
                experience,
                effort: 1 - experience,
                xp: 1,
            }
        })
        .collect()
}

/// Native receipt publication already checked the current source and adopted
/// execution in the graph owner transaction. Preserve the same credit algorithm.
pub(crate) fn publish_graph(
    db: &Connection,
    turn: &str,
    receipt: &str,
    presence: BTreeMap<String, Presence>,
    expected_skills: &BTreeSet<String>,
) -> Result<Observation> {
    if db.is_autocommit() {
        return Err(invalid("Practice publication requires a transaction."));
    }
    publish_checked(db, turn, receipt, presence, expected_skills, || {
        Ok(db.query_row("SELECT EXISTS(SELECT 1 FROM conversation_graph_assessments a JOIN turns t ON t.id=a.turn_id WHERE a.id=?1 AND t.id=?2 AND a.kind='skill_assessment' AND t.state NOT IN ('cancelled','invalidated') AND NOT EXISTS(SELECT 1 FROM turns child WHERE child.replaces_turn_id=t.id))",params![receipt,turn],|r|r.get(0))?)
    })
}

fn publish_checked(
    tx: &Connection,
    turn: &str,
    attempt: &str,
    presence: BTreeMap<String, Presence>,
    expected_skills: &BTreeSet<String>,
    eligible: impl FnOnce() -> Result<bool>,
) -> Result<Observation> {
    if expected_skills.is_empty()
        || presence.keys().any(|k| k.is_empty())
        || presence.keys().cloned().collect::<BTreeSet<_>>() != *expected_skills
    {
        return Err(invalid(
            "Skill presence does not match the captured catalog.",
        ));
    }
    let (raw,text,parent,conversation):(String,String,Option<String>,String)=tx.query_row(
        "SELECT t.context,m.text,t.replaces_turn_id,t.conversation_id FROM turns t JOIN messages m ON m.turn_id=t.id AND m.role='user' WHERE t.id=?1",[turn],
        |r|Ok((r.get(0)?,r.get(1)?,r.get(2)?,r.get(3)?)))?;
    let current: serde_json::Value = serde_json::from_str(&raw)?;
    if let Some(saved) = current.get("practiceObservation") {
        let saved: Observation = serde_json::from_value(saved.clone())?;
        if saved.turn_id != turn
            || saved.inference_attempt_id != attempt
            || saved.presence != presence
            || saved.policy != POLICY
        {
            return Err(invalid(
                "Conflicting practice publication for this submission.",
            ));
        }
        return Ok(saved);
    }
    let eligible = eligible()?;
    if !eligible {
        return Err(invalid(
            "Practice source or inference attempt is no longer current.",
        ));
    }
    let variety = current["practiceSettings"]["varietyId"]
        .as_str()
        .ok_or_else(|| invalid("Missing captured practice variety."))?;
    let mut next = parent;
    let mut seen = BTreeSet::from([turn.to_owned()]);
    let mut encountered = BTreeSet::new();
    let mut changed = true;
    let mut immediate = true;
    while let Some(id) = next {
        if !seen.insert(id.clone()) {
            return Err(invalid("Cyclic practice revision history."));
        }
        let prior:Option<(String,String,Option<String>,String)>=tx.query_row("SELECT t.context,m.text,t.replaces_turn_id,t.conversation_id FROM turns t JOIN messages m ON m.turn_id=t.id AND m.role='user' WHERE t.id=?1",[&id],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?,r.get(3)?))).optional()?;
        let (raw, previous, parent, owner) =
            prior.ok_or_else(|| invalid("Missing practice revision source."))?;
        let value: serde_json::Value = serde_json::from_str(&raw)?;
        if owner != conversation || value["practiceSettings"]["varietyId"] != variety {
            return Err(invalid(
                "Practice revision crosses its conversation or variety.",
            ));
        }
        if immediate {
            changed = text != previous;
            immediate = false;
        }
        if let Some(saved) = value.get("practiceObservation") {
            let saved: Observation = serde_json::from_value(saved.clone())?;
            if saved.policy != POLICY || saved.turn_id != id {
                return Err(invalid("Invalid prior practice observation."));
            }
            encountered.extend(saved.credits.into_iter().map(|c| c.skill_id));
        }
        next = parent;
    }
    let observation = Observation {
        policy: POLICY.into(),
        turn_id: turn.into(),
        inference_attempt_id: attempt.into(),
        credits: credits(&presence, &encountered, changed),
        presence,
    };
    tx.execute(
        "UPDATE turns SET context=json_set(context,'$.practiceObservation',json(?2)) WHERE id=?1",
        params![turn, serde_json::to_string(&observation)?],
    )?;
    Ok(observation)
}

#[cfg(test)]
#[path = "practice_tests.rs"]
mod tests;
