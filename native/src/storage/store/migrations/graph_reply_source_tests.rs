use super::*;

#[test]
fn reserved_reply_sources_preserve_history_and_roll_back_with_the_chain() {
    let dir = tempfile::tempdir().unwrap();
    let mut db = baseline(&dir.path().join("source"), true);
    run_chain(
        &mut db,
        45,
        61,
        &STEPS[..16],
        v61_graph_reply_roles::validate,
    )
    .unwrap();
    db.execute_batch("PRAGMA foreign_keys=ON;
        INSERT INTO personas VALUES('p','learner','english',1,'{}');
        INSERT INTO contacts VALUES('c','learner','p',0,1);
        INSERT INTO conversations VALUES('conversation','c','english','retained',0,1,'then',1);
        INSERT INTO graph_engines VALUES('engine','conversation',printf('%064d',1),'{}',X'010203');
        INSERT INTO turns(id,conversation_id,state,paused,profile_revision,credential_id,route,model,context) VALUES('turn','conversation','succeeded',0,1,'reference','hosted','captured','{\"practiceSettings\":{\"varietyId\":\"original\"}}');
        INSERT INTO turn_execution_owners VALUES('turn','graph','coach','engine','run','artifact');
        INSERT INTO messages(id,conversation_id,turn_id,sequence,role,text) VALUES('message','conversation','turn',1,'assistant','Retained café 日本語');
        INSERT INTO conversation_graph_effects VALUES('effect','turn','reply','text','coach_reply','scope','english','original','graph-effect:effect');
        INSERT INTO conversation_graph_publications VALUES('effect','1','2','message');").unwrap();
    let tables = [
        "messages",
        "conversation_graph_effects",
        "conversation_graph_publications",
        "effort_awards",
    ];
    let before: Vec<_> = tables.iter().map(|table| rows(&db, table)).collect();
    assert!(
        run_chain(&mut db, 61, 62, &STEPS[..17], |_| Err(AppError::new(
            ErrorCode::Storage,
            "injected"
        )))
        .is_err()
    );
    assert_eq!(version(&db), 61);
    assert!(
        db.prepare("SELECT * FROM conversation_graph_reply_sources")
            .is_err()
    );
    run_chain(
        &mut db,
        61,
        62,
        &STEPS[..17],
        v62_graph_reply_sources::validate,
    )
    .unwrap();
    assert_eq!(
        tables
            .iter()
            .map(|table| rows(&db, table))
            .collect::<Vec<_>>(),
        before
    );
    assert!(rows(&db, "conversation_graph_reply_sources").is_empty());
    assert!(
        db.execute(
            "INSERT INTO conversation_graph_reply_sources VALUES('effect','new')",
            []
        )
        .is_err()
    );
    db.execute_batch("INSERT INTO turns(id,conversation_id,state,paused,profile_revision,credential_id,route,model,context)
        SELECT 'next-turn',conversation_id,'pending',paused,profile_revision,credential_id,route,model,context FROM turns WHERE id='turn';
        INSERT INTO turn_execution_owners VALUES('next-turn','graph','coach','engine','next-run','artifact');
        INSERT INTO conversation_graph_effects VALUES('next','next-turn','reply','text','coach_reply','scope','english','original','graph-effect:next');").unwrap();
    assert!(
        db.execute(
            "INSERT INTO conversation_graph_reply_sources VALUES('next','message')",
            []
        )
        .is_err()
    );
    db.execute(
        "INSERT INTO conversation_graph_reply_sources VALUES('next','reserved')",
        [],
    )
    .unwrap();
    assert!(
        db.execute(
            "UPDATE conversation_graph_reply_sources SET message_id='changed'",
            []
        )
        .is_err()
    );
    assert!(
        db.execute(
            "INSERT INTO conversation_graph_publications VALUES('next','3','4','message')",
            []
        )
        .is_err()
    );
    db.execute("INSERT INTO messages(id,conversation_id,turn_id,sequence,role,text) VALUES('reserved','conversation','next-turn',2,'assistant','New reply')",[]).unwrap();
    db.execute(
        "INSERT INTO conversation_graph_publications VALUES('next','3','4','reserved')",
        [],
    )
    .unwrap();
    assert!(
        !db.prepare("PRAGMA foreign_key_check")
            .unwrap()
            .exists([])
            .unwrap()
    );
    assert_eq!(
        include_str!("v62_graph_reply_sources.sql").replace("\r\n", "\n"),
        include_str!("../../schemas/graph_reply_sources.sql").replace("\r\n", "\n")
    );
}
