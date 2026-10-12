use super::*;

#[test]
fn every_historical_cut_uses_native_states_and_complete_topology_without_handlers() {
    let dir = tempfile::tempdir().unwrap();
    let mut store = SqlStore::open(&dir.path().join("owner.db"));
    let graph = graph();
    let engine = trace(&graph);
    let checkpoint = save(&engine, &mut store);
    let original = store.bytes();
    let mut expected = Engine::new([graph.clone()]).unwrap();
    for (index, event) in engine.journal().iter().enumerate() {
        expected.apply(event.clone()).unwrap();
        let revision = (index + 1) as u64;
        let history = checkpoint
            .historical_inspection(revision, read_limits(), &mut ReadOnly(&mut store))
            .unwrap();
        let stamp = Stamp {
            revision,
            ..checkpoint.stamp().clone()
        };
        let live = expected.project(&stamp, "a", export(100)).unwrap();
        let mut all = BTreeMap::<String, Vec<serde_json::Value>>::new();
        let mut cursor = None;
        let mut count = 0;
        loop {
            let page = history.page("a", cursor.as_ref(), export(1)).unwrap();
            assert_eq!(
                header(serde_json::to_value(&page).unwrap()),
                header(serde_json::to_value(&live).unwrap())
            );
            assert_eq!(page.nodes.len(), 3);
            assert_eq!(page.nodes["never"], Disposition::Disabled);
            assert_eq!(page.attempts.offset, count.to_string());
            for record in &page.attempts.records {
                all.entry(record.node.clone())
                    .or_default()
                    .push(serde_json::to_value(&record.attempt).unwrap());
                count += 1;
            }
            cursor = page.attempts.next;
            if cursor.is_none() {
                assert_eq!(page.attempts.total, count.to_string());
                break;
            }
            assert!(count < 100);
        }
        let expected_attempts: BTreeMap<_, _> = live
            .attempts
            .iter()
            .filter(|(_, attempts)| !attempts.is_empty())
            .map(|(node, attempts)| {
                (
                    node.clone(),
                    attempts
                        .iter()
                        .map(|a| serde_json::to_value(a).unwrap())
                        .collect::<Vec<_>>(),
                )
            })
            .collect();
        assert_eq!(all, expected_attempts);
        for (id, producer) in expected.state.executions.iter() {
            let receipt = history.producer_receipt(*id).unwrap();
            assert_eq!(receipt.dispatched, producer.dispatched);
            assert_eq!(receipt.unknown, producer.unknown);
            assert_eq!(
                receipt.outcome,
                producer
                    .outcome
                    .as_ref()
                    .map(|v| v.as_ref().map(|_| ()).map_err(Clone::clone))
            );
        }
    }
    drop(engine);
    drop(expected);
    drop(graph);
    let history = checkpoint
        .historical_inspection(
            checkpoint.stamp().revision,
            read_limits(),
            &mut ReadOnly(&mut store),
        )
        .unwrap();
    let page = history.page("a", None, export(100)).unwrap();
    assert_eq!(page.nodes["second"], Disposition::Adopted);
    assert!(!page.paused); // Historical inspection must not synthesize Recover.
    let json = serde_json::to_string(&page).unwrap();
    for secret in [
        "secret-code",
        "secret-content",
        "123456789",
        "test-authority-v1",
    ] {
        assert!(!json.contains(secret));
    }
    assert!(json.contains("Unclassified"));
    assert!(json.contains("Content"));
    assert_eq!(store.bytes(), original);
    assert!(store.publications().is_empty());
}

#[test]
fn cursor_and_selected_cut_survive_compaction_and_new_events() {
    let dir = tempfile::tempdir().unwrap();
    let mut store = SqlStore::open(&dir.path().join("owner.db"));
    let graph = graph();
    let mut engine = trace(&graph);
    let old = save(&engine, &mut store);
    let cut = old.stamp().revision;
    let history = old
        .historical_inspection(cut, read_limits(), &mut store)
        .unwrap();
    let first = history.page("a", None, export(1)).unwrap();
    let cursor = first.attempts.next.unwrap();
    let expected =
        serde_json::to_value(history.page("a", Some(&cursor), export(1)).unwrap()).unwrap();
    let compacted = compact(&mut engine, &old, &mut store);
    engine
        .apply(Event::Cancel {
            run: "a".into(),
            node: None,
        })
        .unwrap();
    let appended = compacted.capture_next(&engine, bounds()).unwrap();
    store
        .commit(CommitRequest {
            expected: Some(compacted.stamp()),
            next: &appended,
            intent: CommitIntent::Record,
            records: RecordChanges::between(None, &engine.state),
        })
        .unwrap();
    let latest = compact(&mut engine, &appended, &mut store);
    let reopened = latest
        .historical_inspection(cut, read_limits(), &mut store)
        .unwrap();
    assert_eq!(
        serde_json::to_value(reopened.page("a", Some(&cursor), export(1)).unwrap()).unwrap(),
        expected
    );
    assert_eq!(
        serde_json::to_value(history.page("a", Some(&cursor), export(1)).unwrap()).unwrap(),
        expected
    );
    let current = latest
        .historical_inspection(latest.stamp().revision, read_limits(), &mut store)
        .unwrap();
    assert_eq!(
        current
            .page("a", Some(&cursor), export(1))
            .err()
            .unwrap()
            .code,
        "history_cursor"
    );
    assert!(!current.page("a", None, export(1)).unwrap().active);
}

#[test]
fn zero_attempts_are_complete_and_empty_pages_do_not_hide_on_demand_nodes() {
    let dir = tempfile::tempdir().unwrap();
    let mut store = SqlStore::open(&dir.path().join("owner.db"));
    let graph = graph();
    let mut engine = Engine::new([graph.clone()]).unwrap();
    begin(&mut engine, &graph, "a", 40);
    let checkpoint = save(&engine, &mut store);
    let before = checkpoint
        .historical_inspection(0, read_limits(), &mut store)
        .unwrap();
    assert_eq!(
        before.page("a", None, export(1)).err().unwrap().code,
        "unknown_run"
    );
    let history = checkpoint
        .historical_inspection(1, read_limits(), &mut store)
        .unwrap();
    let page = history.page("a", None, export(1)).unwrap();
    assert_eq!(page.attempts.total, "0");
    assert_eq!(page.attempts.offset, "0");
    assert!(page.attempts.next.is_none());
    assert!(page.attempts.records.is_empty());
    assert_eq!(page.nodes["second"], Disposition::Unrequested);
    assert_eq!(page.artifact.definition.nodes.len(), 3);
}

#[test]
fn nested_structure_is_the_exact_recorded_artifact_after_handlers_are_dropped() {
    let dir = tempfile::tempdir().unwrap();
    let mut store = SqlStore::open(&dir.path().join("owner.db"));
    let registry = registry();
    let mut child = registry.compile(definition()).unwrap();
    for prefix in ["inner", "outer"] {
        let mut parent = definition();
        parent.contract = contract(prefix);
        parent.nodes.clear();
        parent.results = parent
            .compose(
                prefix,
                &child,
                BTreeMap::from([("value".into(), Source::Input("value".into()))]),
            )
            .unwrap();
        child = registry.compile(parent).unwrap();
    }
    let graph = Arc::new(child);
    let mut engine = Engine::new([graph.clone()]).unwrap();
    begin(&mut engine, &graph, "a", 40);
    let checkpoint = save(&engine, &mut store);
    let live = engine.project(checkpoint.stamp(), "a", export(10)).unwrap();
    drop(engine);
    drop(graph);
    drop(registry);
    let history = checkpoint
        .historical_inspection(1, read_limits(), &mut store)
        .unwrap();
    let page = history.page("a", None, export(1)).unwrap();
    assert_eq!(
        header(serde_json::to_value(page).unwrap()),
        header(serde_json::to_value(live).unwrap())
    );
}
