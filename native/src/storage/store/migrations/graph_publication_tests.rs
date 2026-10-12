use super::*;

#[test]
fn publication_upgrade_is_additive_rolls_back_and_matches_fresh_schema() {
    let dir = tempfile::tempdir().unwrap();
    let mut db = baseline(&dir.path().join("source"), true);
    run_chain(&mut db, 45, 58, &STEPS[..13], |_| Ok(())).unwrap();
    db.execute_batch("INSERT INTO personas VALUES('p','learner','english',1,'{}'); INSERT INTO contacts VALUES('c','learner','p',0,1); INSERT INTO conversations VALUES('conversation','c','english','retained',0,1,'then',1);
        INSERT INTO turns(id,conversation_id,state,paused,profile_revision,credential_id,route,model,context) VALUES('turn','conversation','succeeded',0,1,'reference','hosted','captured','{}');
        INSERT INTO turn_execution_owners VALUES('turn','legacy','coach',NULL,NULL,NULL);
        INSERT INTO messages(id,conversation_id,turn_id,sequence,role,text) VALUES('message','conversation','turn',1,'assistant','Retained café 日本語');").unwrap();
    let tables = [
        "turns",
        "turn_execution_owners",
        "messages",
        "effort_awards",
    ];
    let before: Vec<_> = tables.iter().map(|t| rows(&db, t)).collect();
    assert!(
        run_chain(&mut db, 58, 59, &STEPS[..14], |_| Err(AppError::new(
            ErrorCode::Storage,
            "injected"
        )))
        .is_err()
    );
    assert_eq!(version(&db), 58);
    assert!(
        db.prepare("SELECT * FROM conversation_graph_effects")
            .is_err()
    );
    assert_eq!(
        tables.iter().map(|t| rows(&db, t)).collect::<Vec<_>>(),
        before
    );
    run_chain(
        &mut db,
        58,
        59,
        &STEPS[..14],
        v59_graph_publications::validate,
    )
    .unwrap();
    assert_eq!(
        tables.iter().map(|t| rows(&db, t)).collect::<Vec<_>>(),
        before
    );
    assert!(rows(&db, "conversation_graph_effects").is_empty());
    assert!(rows(&db, "conversation_graph_publications").is_empty());
    let fresh = Store::open(&dir.path().join("fresh")).unwrap();
    schema::validate_current_schema(&fresh.connection).unwrap();
    v59_graph_publications::validate(&db).unwrap();
}
