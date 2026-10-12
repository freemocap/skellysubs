use super::*;

fn content_chunks(db: &Connection) -> Vec<(String, i64, String)> {
    db.prepare(
        "SELECT record_id,sequence,blob_digest FROM ai_content_chunks ORDER BY record_id,sequence",
    )
    .unwrap()
    .query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))
    .unwrap()
    .collect::<rusqlite::Result<_>>()
    .unwrap()
}

#[test]
fn reference_indexes_preserve_originals_and_rollback_then_reopen() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("workspace.sqlite3");
    let mut db = baseline(&path, true);
    run_chain(&mut db, 45, 74, &STEPS[..29], v74_content::validate).unwrap();
    let before = rows(&db, "learner");
    // References exercise actual retained content through the upgrade, not only DDL.
    db.execute_batch("INSERT INTO drill_items(id,language_id,variety_id,explanation_language,explanation_variety_id,text)
        VALUES('item','spanish','spanish-spain','english','english-united-states','retained');
        INSERT INTO transcription_attempts(id,drill_item_id,route,model,profile_revision,state)
        VALUES('recording','item','custom','fixture',1,'succeeded');
        INSERT INTO inference_blobs VALUES('fixture',x'010203');
        INSERT INTO ai_content_records(id,recording_id,kind,media_type,state,availability,byte_count,last_used)
        VALUES('original','recording','source','application/octet-stream','complete','retained',3,1);
        INSERT INTO ai_content_chunks VALUES('original',0,'fixture');").unwrap();
    let originals = rows(&db, "ai_content_records");
    let chunks = content_chunks(&db);
    assert!(
        run_chain(&mut db, 74, 75, STEPS, |_| Err(AppError::new(
            ErrorCode::Storage,
            "injected"
        )))
        .is_err()
    );
    assert_eq!(version(&db), 74);
    assert_eq!(
        db.query_row(
            "SELECT count(*) FROM sqlite_master WHERE name='ai_content_chunk_blob'",
            [],
            |r| r.get::<_, i32>(0)
        )
        .unwrap(),
        0
    );
    run_chain(&mut db, 74, 75, STEPS, v75_blob_references::validate).unwrap();
    assert_eq!(rows(&db, "learner"), before);
    assert_eq!(rows(&db, "ai_content_records"), originals);
    assert_eq!(content_chunks(&db), chunks);
    drop(db);
    let store = Store::open(&path).unwrap();
    assert_eq!(version(&store.connection), SCHEMA_VERSION);
    v75_blob_references::validate(&store.connection).unwrap();
    assert_eq!(rows(&store.connection, "ai_content_records"), originals);
    assert_eq!(content_chunks(&store.connection), chunks);
    drop(store);
    assert!(Store::open(&path).is_ok());
}
