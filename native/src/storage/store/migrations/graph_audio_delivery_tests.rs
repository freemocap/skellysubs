use super::*;

#[test]
fn native_delivery_upgrade_preserves_legacy_and_rolls_back() {
    let dir = tempfile::tempdir().unwrap();
    let mut db = baseline(&dir.path().join("delivery"), true);
    run_chain(&mut db, 45, 64, &STEPS[..19], v64_graph_audio::validate).unwrap();
    db.execute_batch("INSERT INTO inference_executions(id,task,state,metadata) VALUES('saved','speech','succeeded','{\"request_id\":\"retained\"}');").unwrap();
    let old = rows(&db, "inference_executions");
    assert!(
        run_chain(&mut db, 64, 65, &STEPS[..20], |_| Err(AppError::new(
            ErrorCode::Storage,
            "injected"
        )))
        .is_err()
    );
    assert_eq!(version(&db), 64);
    assert!(db.prepare("SELECT * FROM graph_audio_deliveries").is_err());
    run_chain(
        &mut db,
        64,
        65,
        &STEPS[..20],
        v65_graph_audio_delivery::validate,
    )
    .unwrap();
    assert_eq!(old, rows(&db, "inference_executions"));
    assert!(rows(&db, "graph_audio_deliveries").is_empty());
    assert_eq!(
        include_str!("v65_graph_audio_delivery.sql").replace("\r\n", "\n"),
        include_str!("../../schemas/graph_audio_delivery.sql").replace("\r\n", "\n")
    );
}
