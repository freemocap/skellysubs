//! Calendar arithmetic is independent of storage, providers and presentation.
use crate::model::{AppError, ErrorCode, Result};

#[derive(Clone, Copy, Debug)]
pub(super) struct DayRun {
    pub day: i32,
    pub count: u32,
}

pub(super) fn current(last: Option<DayRun>, today: i32) -> u32 {
    last.filter(|last| i64::from(today) - i64::from(last.day) <= 1)
        .map_or(1, |last| last.count)
}

/// Backward/repeated dates never mint activity. Skipped dates start a new run.
pub(super) fn advance(last: Option<DayRun>, today: i32) -> Result<Option<DayRun>> {
    if last.is_some_and(|last| today <= last.day) {
        return Ok(None);
    }
    let count = match last {
        Some(last) if i64::from(today) - i64::from(last.day) == 1 => last
            .count
            .checked_add(1)
            .ok_or_else(|| AppError::new(ErrorCode::Storage, "Streak capacity exceeded."))?,
        _ => 1,
    };
    Ok(Some(DayRun { day: today, count }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dates_not_elapsed_hours_define_the_run() {
        let last = Some(DayRun { day: 100, count: 7 });
        assert_eq!(current(last, 100), 7);
        assert_eq!(current(last, 101), 7);
        assert_eq!(advance(last, 101).unwrap().unwrap().count, 8);
        assert_eq!(current(last, 102), 1);
        assert_eq!(advance(last, 102).unwrap().unwrap().count, 1);
        assert_eq!(advance(None, 100).unwrap().unwrap().count, 1);
        assert_eq!(current(None, 100), 1);
    }

    #[test]
    fn clock_or_travel_backwards_preserves_but_never_extends() {
        let last = Some(DayRun { day: 100, count: 7 });
        for today in [90, 99, 100] {
            assert_eq!(current(last, today), 7);
            assert!(advance(last, today).unwrap().is_none());
        }
    }
}
