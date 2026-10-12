use super::*;

#[test]
fn reply_role_upgrade_preserves_publications_attribution_and_rollback() {
    let dir = tempfile::tempdir().unwrap();
    let mut db = baseline(&dir.path().join("source"), true);
    run_chain(&mut db, 45, 60, &STEPS[..15], v60_graph_transport::validate).unwrap();
    db.execute_batch("PRAGMA foreign_keys=ON;
        INSERT INTO personas VALUES('p','learner','english',1,'{}');
        INSERT INTO contacts VALUES('c','learner','p',0,1);
        INSERT INTO conversations VALUES('conversation','c','english','retained',0,1,'then',1);
        INSERT INTO graph_engines VALUES('engine','conversation',printf('%064d',1),'{}',X'010203');
        INSERT INTO turns(id,conversation_id,state,paused,profile_revision,credential_id,route,model,context) VALUES('turn','conversation','succeeded',0,1,'reference','hosted','captured','{\"practiceSettings\":{\"varietyId\":\"original\"}}');
        INSERT INTO turn_execution_owners VALUES('turn','graph','coach','engine','run','artifact');
        INSERT INTO messages(id,conversation_id,turn_id,sequence,role,text) VALUES('message','conversation','turn',1,'assistant','Retained café 日本語');
        INSERT INTO conversation_graph_effects VALUES('effect','turn','reply','text','coach_reply','scope','english','original','graph-effect:effect');
        INSERT INTO conversation_graph_publications VALUES('effect','1','2','message');
        UPDATE turns SET context=json_set(context,'$.practiceSettings.varietyId','changed');").unwrap();
    let tables = [
        "turns",
        "turn_execution_owners",
        "graph_engines",
        "messages",
        "conversation_graph_effects",
        "conversation_graph_publications",
        "effort_awards",
    ];
    let before: Vec<_> = tables.iter().map(|t| rows(&db, t)).collect();
    assert!(
        run_chain(&mut db, 60, 61, &STEPS[..16], |_| Err(AppError::new(
            ErrorCode::Storage,
            "injected"
        )))
        .is_err()
    );
    assert_eq!(version(&db), 60);
    v60_graph_transport::validate(&db).unwrap();
    assert_eq!(
        tables.iter().map(|t| rows(&db, t)).collect::<Vec<_>>(),
        before
    );
    run_chain(
        &mut db,
        60,
        61,
        &STEPS[..16],
        v61_graph_reply_roles::validate,
    )
    .unwrap();
    assert_eq!(
        tables.iter().map(|t| rows(&db, t)).collect::<Vec<_>>(),
        before
    );
    assert!(
        !db.prepare("PRAGMA foreign_key_check")
            .unwrap()
            .exists([])
            .unwrap()
    );
    assert_eq!(
        include_str!("v61_graph_reply_roles.sql"),
        include_str!("../../schemas/graph_publications.sql")
    );
    assert!(db.execute("INSERT INTO conversation_graph_effects VALUES('wrong','turn','other','text','persona_reply','scope','english','changed','graph-effect:wrong')",[]).is_err());
    assert!(db.execute("INSERT INTO conversation_graph_effects VALUES('stale','turn','other','text','coach_reply','scope','english','original','graph-effect:stale')",[]).is_err());
    assert!(
        db.execute(
            "UPDATE conversation_graph_effects SET role='persona_reply'",
            []
        )
        .is_err()
    );
}
