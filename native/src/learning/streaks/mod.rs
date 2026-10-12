//! Workspace-owned activity days, independent of skill evidence and XP.
pub(crate) mod commands;
mod rules;
#[cfg(test)]
mod tests;

use crate::model::Result;
use rules::{DayRun, advance, current};
use rusqlite::{Connection, OptionalExtension, params};

#[derive(Clone, Copy)]
pub(crate) struct Clock {
    day: i32,
    timestamp: i64,
    offset: i32,
}

impl Clock {
    /// One SQLite statement fixes the instant for app activity and captures the OS local date.
    pub(crate) fn now(db: &Connection) -> Result<Self> {
        Ok(db.query_row("SELECT CAST(julianday(date('now','localtime'))-2440587.5 AS INTEGER),unixepoch('now'),unixepoch('now','localtime')-unixepoch('now')", [], |row| Ok(Self { day: row.get(0)?, timestamp: row.get(1)?, offset: row.get(2)? }))?)
    }
}

fn last(db: &Connection) -> Result<Option<DayRun>> {
    // Format 72 used a scope column. Keep its on-disk contract readable, but
    // runtime tracking now exclusively owns the app row (the empty scope).
    Ok(db
        .query_row(
            "SELECT day,run_length FROM streak_days WHERE scope='' ORDER BY day DESC LIMIT 1",
            [],
            |row| {
                Ok(DayRun {
                    day: row.get(0)?,
                    count: row.get(1)?,
                })
            },
        )
        .optional()?)
}

/// Caller owns the transaction; recording must commit or roll back with its source.
pub(crate) fn record(db: &Connection, session: &str, clock: Clock) -> Result<()> {
    if let Some(next) = advance(last(db)?, clock.day)? {
        db.execute("INSERT INTO streak_days(scope,day,run_length,occurred_at,utc_offset_seconds,session_id,policy) VALUES('',?1,?2,?3,?4,?5,'daily-streak-1')", params![next.day, next.count, clock.timestamp, clock.offset, session])?;
    }
    Ok(())
}
