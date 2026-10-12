use super::*;

#[tokio::test]
async fn adopted_node_output_is_independent_of_pending_branches_and_residency() {
    let dir = tempfile::tempdir().unwrap();
    let mut store = SqlStore::open(&dir.path().join("owner.db"));
    let mut definition = definition();
    definition.nodes.get_mut("second").unwrap().activation = Activation::OnDemand;
    let graph = Arc::new(registry().compile(definition).unwrap());
    let mut host = DurableEngine::create([graph.clone()], limits(), &mut store).unwrap();
    start(&mut host, &mut store, &graph);
    assert_eq!(
        host.read_node_input("run", "first", "value", &mut store)
            .unwrap(),
        Some(json!(40))
    );
    assert_eq!(
        host.read_node_input("run", "second", "value", &mut store)
            .unwrap(),
        None
    );
    assert!(
        host.read_node_input("run", "first", "missing", &mut store)
            .is_err()
    );
    assert!(
        host.read_node_outputs("run", "missing", &mut store)
            .is_err()
    );
    assert_eq!(
        host.read_node_outputs("run", "first", &mut store).unwrap(),
        None
    );
    host.apply(capacity(1), &mut store).unwrap();
    let attempt = id(&host, "first");
    let invocation = host.claim("run", "first", attempt, &mut store).unwrap();
    host.settle_report(&invocation.execute(evidence_limits()).await, &mut store)
        .unwrap();
    assert_eq!(
        host.read_node_outputs("run", "first", &mut store).unwrap(),
        None
    );
    host.adopt("run", "first", attempt, &mut store).unwrap();
    for cold in [false, true] {
        if cold {
            host.compact(&mut store).unwrap();
            host.evict_records(&mut store).unwrap();
        }
        assert_eq!(
            host.read_node_input("run", "second", "value", &mut store)
                .unwrap(),
            Some(json!(41))
        );
        assert_eq!(host.read_outputs("run", &mut store).unwrap(), None);
        assert_eq!(
            host.read_node_outputs("run", "second", &mut store).unwrap(),
            None
        );
        assert_eq!(
            host.read_node_outputs("run", "first", &mut store).unwrap(),
            Some(values(41))
        );
    }
    let recovered =
        DurableEngine::recover(checkpoint(&store), [graph], limits(), &mut store).unwrap();
    assert_eq!(
        recovered
            .read_node_outputs("run", "first", &mut store)
            .unwrap(),
        Some(values(41))
    );
}
