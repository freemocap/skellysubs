use super::*;

#[test]
fn native_audio_upgrade_is_additive_and_rolls_back_atomically() {
    let dir = tempfile::tempdir().unwrap();
    let mut db = baseline(&dir.path().join("audio"), true);
    run_chain(
        &mut db,
        45,
        63,
        &STEPS[..18],
        v63_graph_assessments::validate,
    )
    .unwrap();
    db.execute_batch("INSERT INTO inference_executions(id,task,state,metadata) VALUES('saved','speech','succeeded','{\"request_id\":\"retained\"}'); INSERT INTO inference_blobs VALUES('digest',X'010203'); INSERT INTO inference_results VALUES('saved','key','digest',1);").unwrap();
    let before = [
        rows(&db, "inference_executions"),
        rows(&db, "inference_results"),
        rows(&db, "inference_blobs"),
    ];
    assert!(
        run_chain(&mut db, 63, 64, &STEPS[..19], |_| Err(AppError::new(
            ErrorCode::Storage,
            "injected"
        )))
        .is_err()
    );
    assert_eq!(version(&db), 63);
    assert!(db.prepare("SELECT * FROM graph_audio_receipts").is_err());
    run_chain(&mut db, 63, 64, &STEPS[..19], v64_graph_audio::validate).unwrap();
    assert_eq!(
        before,
        [
            rows(&db, "inference_executions"),
            rows(&db, "inference_results"),
            rows(&db, "inference_blobs")
        ]
    );
    assert!(rows(&db, "graph_audio_receipts").is_empty());
    assert!(rows(&db, "graph_audio_cache").is_empty());
    assert_eq!(
        include_str!("v64_graph_audio.sql").replace("\r\n", "\n"),
        include_str!("../../schemas/graph_audio.sql").replace("\r\n", "\n")
    );
}
