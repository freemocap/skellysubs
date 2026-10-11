use super::*;
use crate::model::{AppError, ErrorCode};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use ts_rs::TS;

#[derive(Clone, Debug, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct StreakCounter {
    pub count: u32,
    pub day: Option<i32>,
    pub celebration: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct StreakSnapshot {
    pub session_id: String,
    pub today: i32,
    pub app: StreakCounter,
}

pub(super) fn counter(db: &Connection, clock: Clock) -> Result<StreakCounter> {
    let last = last(db)?;
    Ok(StreakCounter {
        count: current(last, clock.day),
        day: last.map(|last| last.day),
        celebration: None,
    })
}

/// Only today's app day from this native session may animate. A lost response
/// can skip an effect, never erase a day.
pub(super) fn claim(db: &Connection, session: &str, clock: Clock) -> Result<StreakCounter> {
    let mut value = counter(db, clock)?;
    if value.day == Some(clock.day)
        && db.execute("UPDATE streak_days SET claimed=1 WHERE scope='' AND day=?1 AND session_id=?2 AND claimed=0", params![clock.day, session])? == 1
    {
        value.celebration = Some(format!("app:{}", clock.day));
    }
    Ok(value)
}

/// Foreground UI signals can record activity; timer/read requests cannot.
/// Session identity prevents a stale window writing into a replacement workspace.
#[tauri::command]
pub(crate) fn refresh_streaks(
    state: tauri::State<'_, Arc<crate::application::Application>>,
    window: tauri::WebviewWindow,
    session_id: String,
    active: bool,
) -> Result<StreakSnapshot> {
    let mut store = state.lock()?;
    if store.session_id != session_id {
        return Err(AppError::new(
            ErrorCode::SessionExpired,
            "The workspace session changed.",
        ));
    }
    let foreground = window.label() == "main"
        && window.is_visible().map_err(|error| {
            crate::diagnostics::failures::platform(
                &error,
                "streak_window_visibility",
                &[],
                AppError::new(ErrorCode::Internal, "Cannot inspect window visibility."),
            )
        })?
        && window.is_focused().map_err(|error| {
            crate::diagnostics::failures::platform(
                &error,
                "streak_window_focus",
                &[],
                AppError::new(ErrorCode::Internal, "Cannot inspect window focus."),
            )
        })?;
    let tx = store.connection.transaction()?;
    let clock = Clock::now(&tx)?;
    if active && foreground {
        record(&tx, &session_id, clock)?;
    }
    let app = if foreground {
        claim(&tx, &session_id, clock)?
    } else {
        counter(&tx, clock)?
    };
    let value = StreakSnapshot {
        session_id,
        today: clock.day,
        app,
    };
    tx.commit()?;
    Ok(value)
}
