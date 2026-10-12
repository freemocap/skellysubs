use super::*;

#[test]
fn step_survives_compaction_history_replay_and_recovery_revokes_it() {
    let dir = tempfile::tempdir().unwrap();
    let mut store = SqlStore::open(&dir.path().join("owner.db"));
    let graph = Arc::new(registry().compile(definition()).unwrap());
    let mut host = DurableEngine::create([graph.clone()], limits(), &mut store).unwrap();
    start(&mut host, &mut store, &graph);
    host.apply(
        Event::Pause {
            run: "run".into(),
            paused: true,
        },
        &mut store,
    )
    .unwrap();
    host.apply(Event::Step { run: "run".into() }, &mut store)
        .unwrap();
    assert_eq!(view(&host)["stepping"], "first");
    let cp = checkpoint(&store);
    assert_eq!(
        serde_json::from_slice::<serde_json::Value>(cp.bytes()).unwrap()["payload"]["format"],
        8
    );
    host.compact(&mut store).unwrap();
    assert_eq!(view(&host)["stepping"], "first");
    let mut recovered =
        DurableEngine::recover(checkpoint(&store), [graph], limits(), &mut store).unwrap();
    assert!(view(&recovered)["stepping"].is_null());
    assert_eq!(view(&recovered)["paused"], true);
    assert!(recovered.apply(capacity(1), &mut store).unwrap().is_empty());
    recovered
        .apply(Event::Step { run: "run".into() }, &mut store)
        .unwrap();
    recovered.apply(capacity(1), &mut store).unwrap();
    let attempt = id(&recovered, "first");
    recovered
        .claim("run", "first", attempt, &mut store)
        .unwrap();
}

#[test]
fn step_event_cannot_be_smuggled_into_an_older_format() {
    let dir = tempfile::tempdir().unwrap();
    let mut store = SqlStore::open(&dir.path().join("owner.db"));
    let graph = Arc::new(registry().compile(definition()).unwrap());
    let mut host = DurableEngine::create([graph.clone()], limits(), &mut store).unwrap();
    start(&mut host, &mut store, &graph);
    host.apply(
        Event::Pause {
            run: "run".into(),
            paused: true,
        },
        &mut store,
    )
    .unwrap();
    host.apply(Event::Step { run: "run".into() }, &mut store)
        .unwrap();
    let mut cp = checkpoint(&store);
    let crate::ai::graph::checkpoint_format::Payload::Evidence(p) = &mut cp.envelope.payload else {
        panic!("step format")
    };
    p.format = 7;
    cp.envelope.checksum = crate::ai::graph::compile::digest(&cp.envelope.payload).unwrap();
    assert!(
        Checkpoint::decode(
            &serde_json::to_vec(&cp.envelope).unwrap(),
            limits().checkpoint
        )
        .is_err()
    );
}

#[test]
fn rejected_step_commit_leaves_no_permission_or_partial_record() {
    let dir = tempfile::tempdir().unwrap();
    let mut store = SqlStore::open(&dir.path().join("owner.db"));
    let graph = Arc::new(registry().compile(definition()).unwrap());
    let mut host = DurableEngine::create([graph.clone()], limits(), &mut store).unwrap();
    start(&mut host, &mut store, &graph);
    host.apply(
        Event::Pause {
            run: "run".into(),
            paused: true,
        },
        &mut store,
    )
    .unwrap();
    let before = store.bytes();
    store.fail_record_after = Some(1);
    assert!(
        host.apply(Event::Step { run: "run".into() }, &mut store)
            .is_err()
    );
    assert_eq!(store.bytes(), before);
    assert!(view(&host)["stepping"].is_null());
    store.fail_record_after = None;
    host.apply(Event::Step { run: "run".into() }, &mut store)
        .unwrap();
    host.evict_records(&mut store).unwrap();
    // The permission must also be read from the versioned cold run record.
    host.apply(capacity(1), &mut store).unwrap();
    let attempt = id(&host, "first");
    host.claim("run", "first", attempt, &mut store).unwrap();
}
