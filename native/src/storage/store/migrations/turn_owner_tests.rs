use super::*;

fn source(path: &Path) -> Connection {
    let mut db = baseline(path, true);
    run_chain(&mut db, 45, 57, &STEPS[..12], |_| Ok(())).unwrap();
    db.execute_batch("INSERT INTO personas VALUES('p','learner','english',1,'{}'); INSERT INTO contacts VALUES('c','learner','p',0,1); INSERT INTO conversations VALUES('conversation','c','english','retained',0,1,'then',1);").unwrap();
    for (id, kind) in [
        ("coach", Some("coach_reply")),
        ("reply", Some("persona_reply")),
        ("opening", Some("persona_opening")),
        ("unknown", None),
    ] {
        db.execute("INSERT INTO turns(id,conversation_id,state,paused,profile_revision,credential_id,route,model,context) VALUES(?1,'conversation','unknown',1,1,'reference','hosted','captured','{}')",[id]).unwrap();
        if let Some(kind) = kind {
            db.execute(
                "INSERT INTO operations(id,turn_id,kind,state) VALUES(?1,?1,?2,'unknown')",
                params![id, kind],
            )
            .unwrap();
        }
    }
    db
}

#[test]
fn turn_ownership_backfill_preserves_sources_and_unknowns_at_format_58() {
    let dir = tempfile::tempdir().unwrap();
    let mut db = source(&dir.path().join("source"));
    let turns = rows(&db, "turns");
    let operations = rows(&db, "operations");
    let awards = rows(&db, "effort_awards");
    run_chain(
        &mut db,
        57,
        58,
        &STEPS[..13],
        v58_turn_execution_owners::validate,
    )
    .unwrap();
    assert_eq!(rows(&db, "turns"), turns);
    assert_eq!(rows(&db, "operations"), operations);
    assert_eq!(rows(&db, "effort_awards"), awards);
    let values = db
        .prepare("SELECT turn_id,executor,channel FROM turn_execution_owners ORDER BY turn_id")
        .unwrap()
        .query_map([], |r| {
            Ok((
                r.get::<_, String>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, String>(2)?,
            ))
        })
        .unwrap()
        .collect::<rusqlite::Result<Vec<_>>>()
        .unwrap();
    assert_eq!(
        values,
        vec![
            ("coach".into(), "legacy".into(), "coach".into()),
            ("opening".into(), "legacy".into(), "persona_opening".into()),
            ("reply".into(), "legacy".into(), "persona_reply".into()),
            ("unknown".into(), "legacy".into(), "unknown".into())
        ]
    );
    assert!(rows(&db, "graph_engines").is_empty());
}

#[test]
fn ambiguous_history_and_final_validation_failure_roll_back_owner_migration() {
    for ambiguous in [false, true] {
        let dir = tempfile::tempdir().unwrap();
        let mut db = source(&dir.path().join("source"));
        if ambiguous {
            db.execute("INSERT INTO operations(id,turn_id,kind,state) VALUES('conflict','coach','persona_reply','unknown')",[]).unwrap();
        }
        let before = rows(&db, "operations");
        assert!(
            run_chain(&mut db, 57, 58, &STEPS[..13], |_| Err(AppError::new(
                ErrorCode::Storage,
                "injected"
            )))
            .is_err()
        );
        assert_eq!(version(&db), 57);
        assert_eq!(rows(&db, "operations"), before);
        assert!(db.prepare("SELECT * FROM turn_execution_owners").is_err());
        if ambiguous {
            db.execute("DELETE FROM operations WHERE id='conflict'", [])
                .unwrap();
        }
        run_chain(
            &mut db,
            57,
            58,
            &STEPS[..13],
            v58_turn_execution_owners::validate,
        )
        .unwrap();
    }
}
