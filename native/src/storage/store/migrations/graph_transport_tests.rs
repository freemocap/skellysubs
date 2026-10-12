use super::*;

#[test]
fn transport_identity_upgrade_preserves_graph_bytes_and_rolls_back_atomically() {
    let dir = tempfile::tempdir().unwrap();
    let mut db = baseline(&dir.path().join("source"), true);
    run_chain(
        &mut db,
        45,
        59,
        &STEPS[..14],
        v59_graph_publications::validate,
    )
    .unwrap();
    db.execute_batch("INSERT INTO personas VALUES('p','learner','english',1,'{}'); INSERT INTO contacts VALUES('c','learner','p',0,1); INSERT INTO conversations VALUES('conversation','c','english','retained',0,1,'then',1);
        INSERT INTO graph_engines VALUES('engine','conversation',printf('%064d',1),'{}',X'010203');
        INSERT INTO graph_records VALUES('engine','{}',X'040506');
        INSERT INTO graph_archives VALUES('engine',printf('%064d',2),'{}',X'070809');").unwrap();
    let tables = [
        "conversations",
        "graph_engines",
        "graph_records",
        "graph_archives",
        "effort_awards",
        "messages",
    ];
    let before: Vec<_> = tables.iter().map(|table| rows(&db, table)).collect();
    assert!(
        run_chain(&mut db, 59, 60, &STEPS[..15], |_| Err(AppError::new(
            ErrorCode::Storage,
            "injected final validation failure"
        )))
        .is_err()
    );
    assert_eq!(version(&db), 59);
    assert!(
        db.prepare("SELECT * FROM graph_transport_identities")
            .is_err()
    );
    assert_eq!(
        tables
            .iter()
            .map(|table| rows(&db, table))
            .collect::<Vec<_>>(),
        before
    );
    run_chain(&mut db, 59, 60, &STEPS[..15], v60_graph_transport::validate).unwrap();
    assert_eq!(
        tables
            .iter()
            .map(|table| rows(&db, table))
            .collect::<Vec<_>>(),
        before
    );
    assert!(rows(&db, "graph_transport_identities").is_empty());
    assert_eq!(
        include_str!("v60_graph_transport.sql"),
        include_str!("../../schemas/graph_transport.sql")
    );
    let fresh = Store::open(&dir.path().join("fresh")).unwrap();
    let objects = |db: &Connection| {
        rows_query(
            db,
            "SELECT type,name,replace(sql,char(13),'') FROM sqlite_master WHERE name LIKE 'graph_transport%' ORDER BY name",
        )
    };
    assert_eq!(objects(&db), objects(&fresh.connection));
}

fn rows_query(db: &Connection, sql: &str) -> Vec<(String, String, String)> {
    db.prepare(sql)
        .unwrap()
        .query_map([], |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)))
        .unwrap()
        .collect::<rusqlite::Result<_>>()
        .unwrap()
}
