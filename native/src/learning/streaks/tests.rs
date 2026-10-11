use super::*;

fn db() -> Connection {
    let db = Connection::open_in_memory().unwrap();
    db.execute_batch(include_str!("../../storage/schemas/streaks.sql"))
        .unwrap();
    db
}

fn clock(day: i32) -> Clock {
    Clock {
        day,
        timestamp: i64::from(day) * 86400,
        offset: 0,
    }
}

#[test]
fn retired_language_rows_are_neither_counted_nor_claimed() {
    let db = db();
    db.execute("INSERT INTO streak_days VALUES('retired-language',100,9,8640000,0,'session',0,'daily-streak-1')", []).unwrap();
    assert_eq!(commands::counter(&db, clock(100)).unwrap().count, 1);
    record(&db, "session", clock(100)).unwrap();
    assert_eq!(
        commands::claim(&db, "session", clock(100)).unwrap().count,
        1
    );
    assert_eq!(
        db.query_row(
            "SELECT claimed FROM streak_days WHERE scope='retired-language'",
            [],
            |row| row.get::<_, u32>(0)
        )
        .unwrap(),
        0
    );
    assert_eq!(
        db.query_row(
            "SELECT count(*) FROM streak_days WHERE scope<>''",
            [],
            |row| row.get::<_, u32>(0)
        )
        .unwrap(),
        1
    );
}

#[test]
fn duplicate_activity_and_restarted_sessions_do_not_extend_days() {
    let mut db = db();
    for session in ["first", "first", "second"] {
        let tx = db.transaction().unwrap();
        record(&tx, session, clock(100)).unwrap();
        tx.commit().unwrap();
    }
    assert_eq!(last(&db).unwrap().unwrap().count, 1);
    assert_eq!(
        db.query_row("SELECT count(*) FROM streak_days", [], |r| r
            .get::<_, u32>(0))
            .unwrap(),
        1
    );
    record(&db, "second", clock(101)).unwrap();
    assert_eq!(last(&db).unwrap().unwrap().count, 2);
}

#[test]
fn transaction_failure_does_not_record_activity() {
    let mut db = db();
    {
        let tx = db.transaction().unwrap();
        record(&tx, "session", clock(100)).unwrap();
        // Rollback is owned by the source transaction.
    }
    assert!(last(&db).unwrap().is_none());
}

#[test]
fn claims_are_once_and_never_revalue_or_replay_old_activity() {
    let db = db();
    record(&db, "first", clock(100)).unwrap();
    let first = commands::claim(&db, "first", clock(100)).unwrap();
    assert_eq!(first.count, 1);
    assert!(first.celebration.is_some());
    assert!(
        commands::claim(&db, "first", clock(100))
            .unwrap()
            .celebration
            .is_none()
    );
    record(&db, "first", clock(101)).unwrap();
    assert!(
        commands::claim(&db, "restarted", clock(101))
            .unwrap()
            .celebration
            .is_none()
    );
    assert!(
        commands::claim(&db, "first", clock(102))
            .unwrap()
            .celebration
            .is_none()
    );
    assert_eq!(commands::counter(&db, clock(102)).unwrap().count, 2);
    assert_eq!(commands::counter(&db, clock(103)).unwrap().count, 1);
}

#[test]
fn visible_baseline_does_not_invent_a_qualifying_day() {
    let db = db();
    let initial = commands::counter(&db, clock(100)).unwrap();
    assert_eq!(initial.count, 1);
    assert_eq!(initial.day, None);
    assert!(initial.celebration.is_none());
    assert!(last(&db).unwrap().is_none());
    record(&db, "session", clock(100)).unwrap();
    assert_eq!(last(&db).unwrap().unwrap().count, 1);
    record(&db, "session", clock(101)).unwrap();
    assert_eq!(last(&db).unwrap().unwrap().count, 2);
}

#[test]
fn civil_dates_preserve_offsets_and_ignore_hour_lengths() {
    let db = db();
    record(
        &db,
        "s",
        Clock {
            day: 100,
            timestamp: 100000,
            offset: -18000,
        },
    )
    .unwrap();
    // Spring-forward: adjacent days can be separated by 23 hours.
    record(
        &db,
        "s",
        Clock {
            day: 101,
            timestamp: 182800,
            offset: -14400,
        },
    )
    .unwrap();
    assert_eq!(last(&db).unwrap().unwrap().count, 2);
    record(&db, "s", clock(99)).unwrap();
    assert_eq!(last(&db).unwrap().unwrap().day, 101);
    record(&db, "s", clock(104)).unwrap();
    assert_eq!(last(&db).unwrap().unwrap().count, 1);
    let offsets: Vec<i32> = db
        .prepare("SELECT utc_offset_seconds FROM streak_days ORDER BY day")
        .unwrap()
        .query_map([], |r| r.get(0))
        .unwrap()
        .collect::<rusqlite::Result<_>>()
        .unwrap();
    assert_eq!(offsets, vec![-18000, -14400, 0]);
}
