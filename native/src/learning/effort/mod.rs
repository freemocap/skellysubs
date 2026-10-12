//! Append-only lifetime effort. Source deletion never revokes earned units.
pub(crate) mod bot;
pub(crate) mod exploration;
pub(crate) mod qualification;
pub(crate) mod report;
mod sources;
#[cfg(test)]
mod tests;
use crate::model::Result;
use rusqlite::{Connection, params};
use serde::{Deserialize, Serialize};
pub(crate) use sources::{message, practice, revision};
use std::sync::Arc;
use ts_rs::TS;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
pub enum EffortDimension {
    PartnerUnderstood,
    RevisionsSent,
    PracticeAttempts,
    Explorations,
    Bot,
}
impl EffortDimension {
    fn key(self) -> &'static str {
        match self {
            Self::PartnerUnderstood => "partner_understood",
            Self::RevisionsSent => "revisions_sent",
            Self::PracticeAttempts => "practice_attempts",
            Self::Explorations => "explorations",
            Self::Bot => "bot",
        }
    }
}
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct EffortAward {
    pub id: String,
    pub dimension: EffortDimension,
    pub source_id: String,
    pub language: String,
    pub variety: String,
    pub conversation_id: Option<String>,
    pub policy: String,
    pub created_at: String,
    pub claimed: bool,
}
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct EffortProgress {
    pub target: String,
    pub partner_understood: u32,
    pub revisions_sent: u32,
    pub practice_attempts: u32,
    pub explorations: u32,
    pub bot: u32,
    pub recent: Vec<EffortAward>,
}

/// Borrow the source's transaction: an award cannot outlive a rolled-back source.
/// Identity excludes policy and inference-attempt IDs so retries cannot mint units.
fn award(
    db: &Connection,
    dimension: EffortDimension,
    source: &str,
    language: &str,
    variety: &str,
    conversation: Option<&str>,
) -> Result<()> {
    let policy = if matches!(dimension, EffortDimension::Bot) {
        "bot-engagement-1"
    } else if matches!(dimension, EffortDimension::Explorations) {
        "exploration-generation-1"
    } else if matches!(dimension, EffortDimension::PartnerUnderstood) {
        "jev-understandability-1"
    } else {
        qualification::POLICY
    };
    db.execute("INSERT INTO effort_awards(id,dimension,source_id,language_id,variety_id,conversation_id,policy) VALUES(?1,?2,?3,?4,?5,?6,?7) ON CONFLICT(dimension,source_id) DO NOTHING", params![uuid::Uuid::new_v4().to_string(), dimension.key(), source, language, variety, conversation, policy])?;
    Ok(())
}
/// Whether a recording receipt earned a practice unit, for marking its attempt.
pub(crate) fn practice_counted(db: &Connection, receipt: Option<&str>) -> Result<bool> {
    let Some(receipt) = receipt else {
        return Ok(false);
    };
    Ok(db.query_row(
        "SELECT EXISTS(SELECT 1 FROM effort_awards WHERE dimension=?1 AND source_id=?2)",
        params![EffortDimension::PracticeAttempts.key(), receipt],
        |r| r.get(0),
    )?)
}
pub(crate) fn read(db: &Connection, target: &str) -> Result<EffortProgress> {
    read_scoped(db, target, None)
}
/// Counts and recent awards for a language, optionally narrowed to one conversation.
pub(crate) fn read_scoped(
    db: &Connection,
    target: &str,
    conversation: Option<&str>,
) -> Result<EffortProgress> {
    let mut progress = EffortProgress {
        target: target.into(),
        partner_understood: 0,
        revisions_sent: 0,
        practice_attempts: 0,
        explorations: 0,
        bot: 0,
        recent: vec![],
    };
    let counts = db
        .prepare(
            "SELECT dimension,count(*) FROM effort_awards WHERE dimension!='no_issues_flagged' AND language_id=?1 AND (?2 IS NULL OR conversation_id=?2) GROUP BY dimension",
        )?
        .query_map(params![target, conversation], |r| {
            Ok((r.get::<_, String>(0)?, r.get::<_, u32>(1)?))
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    for (dimension, count) in counts {
        match dimension.as_str() {
            "partner_understood" => progress.partner_understood = count,
            "revisions_sent" => progress.revisions_sent = count,
            "practice_attempts" => progress.practice_attempts = count,
            "explorations" => progress.explorations = count,
            "bot" => progress.bot = count,
            _ => {
                return Err(crate::model::AppError::new(
                    crate::model::ErrorCode::Validation,
                    "Unknown effort dimension.",
                ));
            }
        }
    }
    let rows = db.prepare("SELECT id,dimension,source_id,language_id,variety_id,conversation_id,policy,created_at,claimed FROM effort_awards WHERE dimension!='no_issues_flagged' AND language_id=?1 AND (?2 IS NULL OR conversation_id=?2) ORDER BY rowid DESC LIMIT 100")?.query_map(params![target, conversation], |r| Ok((r.get::<_,String>(0)?,r.get::<_,String>(1)?,r.get::<_,String>(2)?,r.get::<_,String>(3)?,r.get::<_,String>(4)?,r.get::<_,Option<String>>(5)?,r.get::<_,String>(6)?,r.get::<_,String>(7)?,r.get::<_,bool>(8)?)))?.collect::<rusqlite::Result<Vec<_>>>()?;
    for (
        id,
        dimension,
        source_id,
        language,
        variety,
        conversation_id,
        policy,
        created_at,
        claimed,
    ) in rows
    {
        progress.recent.push(EffortAward {
            id,
            dimension: serde_json::from_value(serde_json::Value::String(dimension))?,
            source_id,
            language,
            variety,
            conversation_id,
            policy,
            created_at,
            claimed,
        });
    }
    Ok(progress)
}
#[tauri::command]
pub(crate) fn get_effort_progress(
    state: tauri::State<'_, Arc<crate::application::Application>>,
    target: String,
    conversation: Option<String>,
) -> Result<EffortProgress> {
    state.read_effort_progress(&target, conversation.as_deref())
}
/// Presentation is separate from credit; claiming never changes a total.
#[tauri::command]
pub(crate) fn claim_effort_awards(
    state: tauri::State<'_, Arc<crate::application::Application>>,
    target: String,
    ids: Vec<String>,
) -> Result<Vec<String>> {
    let mut store = state.lock()?;
    store.config.language(&target)?;
    claim(&mut store.connection, &target, &ids)
}
fn claim(db: &mut Connection, target: &str, ids: &[String]) -> Result<Vec<String>> {
    if ids.len() > 100 {
        return Err(crate::model::AppError::new(
            crate::model::ErrorCode::Validation,
            "Too many effort claims.",
        ));
    }
    let tx = db.transaction()?;
    let mut claimed = Vec::new();
    for id in ids {
        if tx.execute(
            "UPDATE effort_awards SET claimed=1 WHERE id=?1 AND language_id=?2 AND dimension!='no_issues_flagged' AND claimed=0",
            params![id, target],
        )? == 1
        {
            claimed.push(id.clone());
        }
    }
    tx.commit()?;
    Ok(claimed)
}
