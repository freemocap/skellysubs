use super::*;

#[test]
fn graph_storage_upgrade_preserves_every_supported_start_and_reopens() {
    for start in 45..SCHEMA_VERSION {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("workspace");
        let mut db = baseline(&path, true);
        run_chain(&mut db, 45, start, &STEPS[..(start - 45) as usize], |_| {
            Ok(())
        })
        .unwrap();
        let awards = rows(&db, "effort_awards");
        let topics = rows(&db, "saved_topics");
        let reading = rows(&db, "reading_attempts");
        drop(db);
        let store = Store::open(&path).unwrap();
        assert_eq!(version(&store.connection), SCHEMA_VERSION);
        assert_eq!(rows(&store.connection, "effort_awards"), awards);
        assert_eq!(rows(&store.connection, "saved_topics"), topics);
        assert_eq!(rows(&store.connection, "reading_attempts"), reading);
        for table in [
            "graph_engines",
            "graph_records",
            "graph_archives",
            "graph_transport_identities",
            "conversation_graph_effects",
            "conversation_graph_publications",
        ] {
            assert!(rows(&store.connection, table).is_empty());
        }
        schema::validate_current_schema(&store.connection).unwrap();
        drop(store);
        let reopened = Store::open(&path).unwrap();
        assert_eq!(version(&reopened.connection), SCHEMA_VERSION);
        assert_eq!(
            std::fs::read_dir(dir.path().join("migration-backups"))
                .unwrap()
                .count(),
            1
        );
    }
}

#[test]
fn graph_storage_migration_rolls_back_and_matches_fresh_schema() {
    let dir = tempfile::tempdir().unwrap();
    let mut db = baseline(&dir.path().join("source"), true);
    run_chain(&mut db, 45, 56, &STEPS[..11], |_| Ok(())).unwrap();
    db.execute_batch("INSERT INTO personas VALUES('p','learner','english',1,'{}');
        INSERT INTO contacts VALUES('c','learner','p',0,1);
        INSERT INTO conversations VALUES('conversation','c','english','Retained',1,1,'then',1);
        INSERT INTO turns(id,conversation_id,state,paused,profile_revision,credential_id,route,model,context) VALUES('turn','conversation','unknown',1,1,'credential-reference','hosted','captured-model','{}');
        INSERT INTO messages(id,conversation_id,turn_id,sequence,role,text) VALUES('message','conversation','turn',1,'user','Retained café 日本語');
        INSERT INTO operations(id,turn_id,kind,state) VALUES('operation','turn','coach_reply','unknown');
        INSERT INTO attempts(id,operation_id,state,requested_model,diagnostics,preview_text) VALUES('attempt','operation','unknown','captured-model','{\"requestId\":\"retained-provider-id\"}','Retained partial source');").unwrap();
    let preserved = [
        "personas",
        "contacts",
        "conversations",
        "turns",
        "messages",
        "operations",
        "attempts",
    ];
    let history: Vec<_> = preserved.iter().map(|table| rows(&db, table)).collect();
    let before = rows(&db, "effort_awards");
    assert!(
        run_chain(&mut db, 56, SCHEMA_VERSION, STEPS, |_| Err(AppError::new(
            ErrorCode::Storage,
            "injected final validation failure"
        )))
        .is_err()
    );
    assert_eq!(version(&db), 56);
    assert!(db.prepare("SELECT * FROM graph_engines").is_err());
    assert_eq!(rows(&db, "effort_awards"), before);
    assert_eq!(
        preserved
            .iter()
            .map(|table| rows(&db, table))
            .collect::<Vec<_>>(),
        history
    );
    run_chain(
        &mut db,
        56,
        SCHEMA_VERSION,
        STEPS,
        schema::validate_current_schema,
    )
    .unwrap();
    assert_eq!(rows(&db, "effort_awards"), before);
    assert_eq!(
        preserved
            .iter()
            .map(|table| rows(&db, table))
            .collect::<Vec<_>>(),
        history
    );
    let fresh = Store::open(&dir.path().join("fresh")).unwrap();
    let objects = |db: &Connection| {
        db.prepare("SELECT type,name,replace(sql,char(13),'') FROM sqlite_master WHERE name LIKE 'graph_%' ORDER BY name").unwrap().query_map([],|r|Ok((r.get::<_,String>(0)?,r.get::<_,String>(1)?,r.get::<_,String>(2)?))).unwrap().collect::<rusqlite::Result<Vec<_>>>().unwrap()
    };
    assert_eq!(objects(&db), objects(&fresh.connection));
}
