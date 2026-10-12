use super::*;

#[test]
fn every_supported_start_preserves_history_and_reaches_streak_format() {
    for start in 45..=71 {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("workspace");
        let mut db = baseline(&path, true);
        if start > 45 {
            let steps = &STEPS[..(start - 45) as usize];
            run_chain(&mut db, 45, start, steps, steps.last().unwrap().validate).unwrap();
        }
        let awards = rows(&db, "effort_awards");
        drop(db);
        let store = Store::open(&path).unwrap();
        assert_eq!(version(&store.connection), SCHEMA_VERSION, "source {start}");
        assert_eq!(rows(&store.connection, "effort_awards"), awards);
        assert!(rows(&store.connection, "streak_days").is_empty());
    }
}

#[test]
fn streak_migration_is_empty_preserves_history_and_rolls_back() {
    let dir = tempfile::tempdir().unwrap();
    let mut db = baseline(&dir.path().join("source"), true);
    run_chain(&mut db, 45, 71, &STEPS[..26], v71_cache::validate).unwrap();
    let before = rows(&db, "effort_awards");
    assert!(
        run_chain(&mut db, 71, 72, &STEPS[..27], |_| Err(AppError::new(
            ErrorCode::Storage,
            "Injected validation failure."
        )))
        .is_err()
    );
    assert_eq!(version(&db), 71);
    assert!(db.prepare("SELECT * FROM streak_days").is_err());
    assert_eq!(rows(&db, "effort_awards"), before);
    run_chain(&mut db, 71, 72, &STEPS[..27], v72_streaks::validate).unwrap();
    assert!(rows(&db, "streak_days").is_empty());
    assert_eq!(rows(&db, "effort_awards"), before);
    assert_eq!(
        include_str!("v72_streaks.sql").replace("\r\n", "\n"),
        include_str!("../../schemas/streaks.sql").replace("\r\n", "\n")
    );
}

#[test]
fn recorded_streaks_survive_reopening_without_new_awards() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("workspace");
    let store = Store::open(&path).unwrap();
    let clock = crate::learning::streaks::Clock::now(&store.connection).unwrap();
    crate::learning::streaks::record(&store.connection, &store.session_id, clock).unwrap();
    let before = rows(&store.connection, "streak_days");
    drop(store);
    let store = Store::open(&path).unwrap();
    assert_eq!(rows(&store.connection, "streak_days"), before);
}
