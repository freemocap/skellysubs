use super::*;

#[test]
fn admission_resident_and_persisted_counts_agree_and_stale_stamp_is_rejected() {
    let (_dir, mut store, conversation) = setup();
    store.execute(send(&store, &conversation)).unwrap();
    let unloaded = graph_runtime::Runtime::new().unwrap();
    let persisted = graph_runtime::outstanding(&store.connection, &unloaded).unwrap();
    assert!(persisted > 0);
    assert_eq!(
        graph_runtime::outstanding(&store.connection, &store.graph_runtime).unwrap(),
        persisted
    );
    let tx = store.connection.unchecked_transaction().unwrap();
    tx.execute(
        "UPDATE graph_engines SET stamp=json_set(stamp,'$.revision',999999)",
        [],
    )
    .unwrap();
    assert!(graph_runtime::outstanding(&tx, &store.graph_runtime).is_err());
    tx.rollback().unwrap();
}

#[test]
#[ignore = "manual timing of resident and persisted admission paths"]
fn admission_current_state_timing() {
    let (_dir, mut store, conversation) = setup();
    store.execute(send(&store, &conversation)).unwrap();
    let unloaded = graph_runtime::Runtime::new().unwrap();
    let expected = graph_runtime::outstanding(&store.connection, &unloaded).unwrap();
    for (label, runtime) in [("persisted", &unloaded), ("resident", &store.graph_runtime)] {
        let mut samples = Vec::new();
        for _ in 0..100 {
            let start = std::time::Instant::now();
            assert_eq!(
                graph_runtime::outstanding(&store.connection, runtime).unwrap(),
                expected
            );
            samples.push(start.elapsed().as_micros());
        }
        samples.sort_unstable();
        println!(
            "admission_us path={label} samples=100 p50={} p95={} max={} build=debug",
            samples[49], samples[94], samples[99]
        );
    }
}

#[tokio::test]
#[ignore = "manual timing; fixture setup is outside measured samples"]
async fn idle_claim_with_retained_history_timing() {
    let (_dir, mut store, conversation) = setup();
    coach_provider(&store, &["A saved coach reply."; 12]);
    for _ in 0..12 {
        store
            .execute(native_coach::ask(&store, &conversation))
            .unwrap();
        for _ in 0..4 {
            native_coach::tick(&mut store).await;
        }
    }
    let mut samples = Vec::new();
    for _ in 0..100 {
        let start = std::time::Instant::now();
        assert!(
            store
                .graph_runtime
                .next(
                    &mut store.connection,
                    true,
                    &store.config,
                    &store.session_id,
                )
                .unwrap()
                .is_none()
        );
        samples.push(start.elapsed().as_micros());
    }
    samples.sort_unstable();
    println!(
        "idle_claim_us retained_coach_runs=12 samples=100 p50={} p95={} max={} build=debug",
        samples[49], samples[94], samples[99]
    );
}
