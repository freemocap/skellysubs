use super::*;

fn source(path: &Path) -> Connection {
    let db = baseline(path, true);
    upgrade_46(&db).unwrap();
    v48_assessment::apply(&db).unwrap();
    db.pragma_update(None, "user_version", 48).unwrap();
    db
}

#[test]
fn execution_preferences_preserve_accepted_work_and_historical_defaults() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("workspace");
    let mut db = source(&path);
    let before = rows(&db, "learner");
    let awards = rows(&db, "effort_awards");
    run_chain(&mut db, 48, 50, &STEPS[..5], v50_coaching::validate).unwrap();
    let value: String = db
        .query_row("SELECT preferences FROM learner", [], |r| r.get(0))
        .unwrap();
    let mut value: serde_json::Value = serde_json::from_str(&value).unwrap();
    assert_eq!(
        value.as_object_mut().unwrap().remove("execution").unwrap(),
        serde_json::json!({"assessment":"automatic", "replyBrief":"on_demand", "reading":"on_demand"})
    );
    assert_eq!(
        before[0][4],
        rusqlite::types::Value::Text(value.to_string())
    );
    assert_eq!(before[0][3], rows(&db, "learner")[0][3]);
    assert_eq!(awards, rows(&db, "effort_awards"));
    let once = rows(&db, "learner");
    run_chain(&mut db, 50, 50, &STEPS[..5], v50_coaching::validate).unwrap();
    assert_eq!(once, rows(&db, "learner"));
}

#[test]
fn execution_preference_migration_rolls_back_and_rejects_malformed_records() {
    let dir = tempfile::tempdir().unwrap();
    let mut db = source(&dir.path().join("workspace"));
    let before = rows(&db, "learner");
    assert!(
        run_chain(&mut db, 48, 50, &STEPS[..5], |_| Err(AppError::new(
            ErrorCode::Storage,
            "Injected final failure."
        )))
        .is_err()
    );
    assert_eq!(version(&db), 48);
    assert_eq!(rows(&db, "learner"), before);
    for bad in [
        "null",
        "{}",
        "{\"assessment\":\"sometimes\",\"coaching\":\"on_demand\",\"replyBrief\":\"on_demand\",\"reading\":\"on_demand\"}",
    ] {
        db.execute(
            "UPDATE learner SET preferences=json_set(preferences,'$.execution',json(?1))",
            [bad],
        )
        .unwrap();
        let malformed = rows(&db, "learner");
        assert!(run_chain(&mut db, 48, 50, &STEPS[..5], v50_coaching::validate).is_err());
        assert_eq!(version(&db), 48);
        assert_eq!(rows(&db, "learner"), malformed);
    }
}

#[test]
fn required_coaching_preserves_optional_choices_and_accepted_work() {
    for coaching in ["automatic", "on_demand"] {
        let dir = tempfile::tempdir().unwrap();
        let mut db = super::assessment::populated(&dir.path().join("workspace"));
        run_chain(&mut db, 47, 49, &STEPS[..4], v49_execution::validate).unwrap();
        db.execute("UPDATE learner SET preferences=json_set(preferences,'$.execution.coaching',?1,'$.execution.assessment','on_demand','$.execution.reading','automatic')", [coaching]).unwrap();
        let retained = ["turns", "operations", "attempts", "effort_awards"];
        let before: Vec<_> = retained.iter().map(|table| rows(&db, table)).collect();
        let learner = rows(&db, "learner");
        assert!(
            run_chain(&mut db, 49, 50, &STEPS[..5], |_| Err(AppError::new(
                ErrorCode::Storage,
                "Fixture rollback"
            )))
            .is_err()
        );
        assert_eq!(version(&db), 49);
        assert_eq!(rows(&db, "learner"), learner);
        run_chain(&mut db, 49, 50, &STEPS[..5], v50_coaching::validate).unwrap();
        let raw: String = db
            .query_row("SELECT preferences FROM learner", [], |r| r.get(0))
            .unwrap();
        let preferences: Preferences = serde_json::from_str(&raw).unwrap();
        use crate::configuration::execution::ExecutionMode;
        assert_eq!(preferences.execution.assessment, ExecutionMode::OnDemand);
        assert_eq!(preferences.execution.reading, ExecutionMode::Automatic);
        assert!(preferences.execution.automatic("coach_feedback"));
        assert_eq!(rows(&db, "learner")[0][3], learner[0][3]);
        for (table, expected) in retained.iter().zip(before) {
            assert_eq!(rows(&db, table), expected);
        }
        let once = rows(&db, "learner");
        run_chain(&mut db, 50, 50, &STEPS[..5], v50_coaching::validate).unwrap();
        assert_eq!(rows(&db, "learner"), once);
    }
}
