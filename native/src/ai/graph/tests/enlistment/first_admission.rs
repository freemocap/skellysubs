use super::*;

fn graph_counts(db: &Connection) -> (i64, i64) {
    (
        db.query_row("SELECT count(*) FROM checkpoint", [], |r| r.get(0))
            .unwrap(),
        db.query_row("SELECT count(*) FROM graph_record", [], |r| r.get(0))
            .unwrap(),
    )
}

#[test]
fn first_admission_is_one_owner_commit_with_no_intermediate_empty_engine() {
    for reject in [false, true] {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("owner.db");
        let mut store = SqlStore::open(&path);
        owner_tables(&store.conn);
        let observer = Connection::open(&path).unwrap();
        let graph = Arc::new(registry().compile(definition()).unwrap());
        let mut enlisted = stage(&mut store.conn, "first");
        enlisted.reject = reject;
        assert_eq!(counts(&observer), (0, 0));
        assert_eq!(graph_counts(&observer), (0, 0));
        let result = DurableEngine::create_with_run(
            [graph.clone()],
            limits(),
            begin_event(&graph),
            &mut enlisted,
        );
        // The one-shot adapter takes its transaction exactly once. A second
        // commit would panic, even if the first happened to contain no records.
        assert!(enlisted.tx.is_none());
        assert_eq!(enlisted.reads, 0);
        drop(enlisted);
        assert_eq!(result.is_err(), reject);
        if reject {
            assert_eq!(counts(&observer), (0, 0));
            assert_eq!(graph_counts(&observer), (0, 0));
        } else {
            let host = result.unwrap();
            assert_eq!(counts(&observer), (1, 1));
            assert_eq!(graph_counts(&observer), (1, 1));
            assert_eq!(host.stamp().revision, 1);
            assert!(
                host.inspect("run")
                    .unwrap()
                    .attempts
                    .values()
                    .all(Vec::is_empty)
            );
            assert!(store.publications().is_empty());
            let checkpoint = Checkpoint::decode(&store.bytes(), limits().checkpoint).unwrap();
            let recovered =
                DurableEngine::recover(checkpoint, [graph], limits(), &mut store).unwrap();
            assert_eq!(
                recovered.inspect("run").unwrap().artifact_id,
                host.inspect("run").unwrap().artifact_id
            );
            assert_eq!(counts(&store.conn), (1, 1));
        }
    }
}

#[test]
fn invalid_first_admission_never_calls_commit_and_owner_drop_rolls_back() {
    for case in 0..6 {
        let dir = tempfile::tempdir().unwrap();
        let mut store = SqlStore::open(&dir.path().join("owner.db"));
        owner_tables(&store.conn);
        let graph = Arc::new(registry().compile(definition()).unwrap());
        let mut event = begin_event(&graph);
        let mut bounds = limits();
        match case {
            0 => {
                if let Event::Begin { inputs, .. } = &mut event {
                    inputs.clear();
                }
            }
            1 => event = capacity(1),
            2 => {
                if let Event::Begin { artifact, .. } = &mut event {
                    *artifact = "absent".into();
                }
            }
            3 => bounds.state.runs = 0,
            4 => bounds.checkpoint.bytes = 1,
            5 => bounds.checkpoint.events = 1,
            _ => unreachable!(),
        }
        {
            let mut enlisted = stage(&mut store.conn, "invalid");
            assert!(DurableEngine::create_with_run([graph], bounds, event, &mut enlisted).is_err());
            assert!(enlisted.tx.is_some(), "case {case} attempted a commit");
        }
        assert_eq!(counts(&store.conn), (0, 0));
        assert_eq!(graph_counts(&store.conn), (0, 0));
    }
}

#[test]
fn first_admission_checks_current_authority_and_rolls_back_partial_record_writes() {
    for case in 0..3 {
        let dir = tempfile::tempdir().unwrap();
        let mut store = SqlStore::open(&dir.path().join("owner.db"));
        owner_tables(&store.conn);
        let graph = Arc::new(registry().compile(definition()).unwrap());
        let mut enlisted = stage(&mut store.conn, "rejected");
        let expected = match case {
            0 => {
                enlisted
                    .tx
                    .as_ref()
                    .unwrap()
                    .execute("UPDATE authority SET scope='revoked'", [])
                    .unwrap();
                "authority_changed"
            }
            1 => {
                enlisted
                    .tx
                    .as_ref()
                    .unwrap()
                    .execute(
                        "UPDATE authority SET input=?1",
                        [serde_json::to_string(&values(41)).unwrap()],
                    )
                    .unwrap();
                "source_changed"
            }
            _ => {
                enlisted.fail_record_after = Some(1);
                "record_write_injected"
            }
        };
        let error = DurableEngine::create_with_run(
            [graph.clone()],
            limits(),
            begin_event(&graph),
            &mut enlisted,
        )
        .err()
        .unwrap();
        assert_eq!(error.code, expected);
        assert!(enlisted.tx.is_none());
        drop(enlisted);
        assert_eq!(counts(&store.conn), (0, 0));
        assert_eq!(graph_counts(&store.conn), (0, 0));
    }
}

#[test]
fn lost_first_commit_ack_is_resolved_by_receipt_and_checkpoint_without_republication() {
    let dir = tempfile::tempdir().unwrap();
    let mut store = SqlStore::open(&dir.path().join("owner.db"));
    owner_tables(&store.conn);
    let graph = Arc::new(registry().compile(definition()).unwrap());
    {
        let mut enlisted = stage(&mut store.conn, "first");
        enlisted.lose_ack = true;
        let result = DurableEngine::create_with_run(
            [graph.clone()],
            limits(),
            begin_event(&graph),
            &mut enlisted,
        );
        assert_eq!(result.err().unwrap().code, "acknowledgment_lost");
    }
    assert_eq!(counts(&store.conn), (1, 1));
    assert_eq!(graph_counts(&store.conn), (1, 1));
    let before = store.bytes();
    // Even an erroneous fresh-create retry cannot overwrite the committed engine.
    {
        let mut enlisted = stage(&mut store.conn, "duplicate");
        let result = DurableEngine::create_with_run(
            [graph.clone()],
            limits(),
            begin_event(&graph),
            &mut enlisted,
        );
        assert_eq!(result.err().unwrap().code, "stale_checkpoint");
    }
    assert_eq!(counts(&store.conn), (1, 1));
    assert_eq!(store.bytes(), before);
    let checkpoint = Checkpoint::decode(&before, limits().checkpoint).unwrap();
    let mut host = DurableEngine::recover(checkpoint, [graph], limits(), &mut store).unwrap();
    assert!(host.inspect("run").is_ok());
    assert!(host.apply(capacity(1), &mut store).unwrap().is_empty());
    host.apply(
        Event::Pause {
            run: "run".into(),
            paused: false,
        },
        &mut store,
    )
    .unwrap();
    assert!(!host.apply(capacity(1), &mut store).unwrap().is_empty());
    assert_eq!(counts(&store.conn), (1, 1));
}

#[test]
fn first_admission_reserves_recovery_at_the_exact_byte_boundary() {
    let dir = tempfile::tempdir().unwrap();
    let graph = Arc::new(registry().compile(definition()).unwrap());
    let mut reference = SqlStore::open(&dir.path().join("reference.db"));
    reference.authorize("run", "scope");
    DurableEngine::create_with_run(
        [graph.clone()],
        limits(),
        begin_event(&graph),
        &mut reference,
    )
    .unwrap();
    let required = reference.bytes().len() + serde_json::to_vec(&Event::Recover).unwrap().len() + 1;
    for short in [false, true] {
        let mut store = SqlStore::open(&dir.path().join(format!("bounded-{short}.db")));
        owner_tables(&store.conn);
        let mut bounds = limits();
        bounds.checkpoint.bytes = required - usize::from(short);
        bounds.checkpoint.events = 2;
        let mut enlisted = stage(&mut store.conn, "bounded");
        let result = DurableEngine::create_with_run(
            [graph.clone()],
            bounds,
            begin_event(&graph),
            &mut enlisted,
        );
        assert_eq!(result.is_err(), short);
        assert_eq!(enlisted.tx.is_some(), short);
        drop(enlisted);
        assert_eq!(counts(&store.conn), if short { (0, 0) } else { (1, 1) });
        if !short {
            let checkpoint = Checkpoint::decode(&store.bytes(), bounds.checkpoint).unwrap();
            DurableEngine::recover(checkpoint, [graph.clone()], bounds, &mut store).unwrap();
            assert_eq!(store.bytes().len(), required);
        }
    }
}
