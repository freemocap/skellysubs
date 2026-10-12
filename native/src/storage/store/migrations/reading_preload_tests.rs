use super::*;

#[test]
fn reading_import_upgrade_preserves_history_from_every_supported_version() {
    for start in 45..=55 {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("workspace");
        let mut db = baseline(&path, true);
        run_chain(&mut db, 45, start, &STEPS[..(start - 45) as usize], |_| {
            Ok(())
        })
        .unwrap();
        let awards = rows(&db, "effort_awards");
        let topics = rows(&db, "saved_topics");
        drop(db);
        let store = Store::open(&path).unwrap();
        assert_eq!(version(&store.connection), SCHEMA_VERSION);
        assert_eq!(rows(&store.connection, "effort_awards"), awards);
        assert_eq!(rows(&store.connection, "saved_topics"), topics);
        assert!(
            store
                .connection
                .query_row("SELECT count(*) FROM reading_dictionary", [], |r| r
                    .get::<_, i64>(0))
                .unwrap()
                > 0
        );
        let packages = rows(&store.connection, "reading_packages");
        let dictionary = rows(&store.connection, "reading_dictionary");
        drop(store);
        let reopened = Store::open(&path).unwrap();
        assert_eq!(rows(&reopened.connection, "reading_packages"), packages);
        assert_eq!(rows(&reopened.connection, "reading_dictionary"), dictionary);
    }
}

#[test]
fn review_upgrade_preserves_imports_and_rolls_back_on_failure() {
    let dir = tempfile::tempdir().unwrap();
    let mut db = baseline(&dir.path().join("workspace"), true);
    run_chain(&mut db, 45, 55, &STEPS[..10], |_| Ok(())).unwrap();
    db.execute_batch("INSERT INTO reading_packages VALUES('retained','1','digest','license','source_checked');
        INSERT INTO reading_dictionary VALUES('retained','entry','english','english-united-states','spanish','spanish-spain','book','{}');").unwrap();
    let packages = rows(&db, "reading_packages");
    let dictionary = rows(&db, "reading_dictionary");
    let awards = rows(&db, "effort_awards");
    assert!(
        run_chain(&mut db, 55, 56, &STEPS[..11], |_| {
            Err(AppError::new(
                ErrorCode::Storage,
                "forced validation failure",
            ))
        })
        .is_err()
    );
    assert_eq!(version(&db), 55);
    assert_eq!(rows(&db, "reading_packages"), packages);
    assert_eq!(rows(&db, "reading_dictionary"), dictionary);
    run_chain(&mut db, 55, 56, &STEPS[..11], |_| Ok(())).unwrap();
    assert_eq!(rows(&db, "reading_packages"), packages);
    assert_eq!(rows(&db, "reading_dictionary"), dictionary);
    assert_eq!(rows(&db, "effort_awards"), awards);
    db.execute(
        "UPDATE reading_packages SET review='editorially_reviewed'",
        [],
    )
    .unwrap();
    assert!(
        db.execute("UPDATE reading_packages SET review='draft'", [])
            .is_err()
    );
    schema::validate_database(&db).unwrap();
}
