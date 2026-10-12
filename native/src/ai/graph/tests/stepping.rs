use super::*;

fn pause(engine: &mut Engine, run: &str) {
    engine
        .apply(Event::Pause {
            run: run.into(),
            paused: true,
        })
        .unwrap();
}
fn step(engine: &mut Engine, run: &str) {
    engine.apply(Event::Step { run: run.into() }).unwrap();
}

#[test]
fn step_selects_one_node_and_never_releases_siblings_or_descendants() {
    let mut definition = definition();
    definition
        .nodes
        .insert("sibling".into(), node(Source::Input("value".into())));
    let graph = Arc::new(registry().compile(definition).unwrap());
    let mut e = Engine::new([graph.clone()]).unwrap();
    begin(&mut e, &graph, "run", 1);
    assert!(e.apply(Event::Step { run: "run".into() }).is_err());
    pause(&mut e, "run");
    step(&mut e, "run");
    assert!(e.apply(Event::Step { run: "run".into() }).is_err());
    assert!(advance(&mut e, capacity(0)).is_empty());
    assert_eq!(e.inspect("run").unwrap().stepping.as_deref(), Some("first"));
    let work = advance(&mut e, capacity(2));
    assert_eq!(work.len(), 1);
    assert_eq!(
        e.disposition("run", "sibling").unwrap(),
        Disposition::Paused
    );
    e.apply(Event::Settle {
        execution: work[0].execution,
        outcome: Ok(values(2)),
    })
    .unwrap();
    adopt(&mut e, "run", "first");
    assert!(e.state.runs["run"].paused);
    assert!(e.inspect("run").unwrap().stepping.is_none());
    assert!(advance(&mut e, capacity(2)).is_empty());
    assert_eq!(e.disposition("run", "second").unwrap(), Disposition::Paused);
    assert!(e.inspect("run").unwrap().step_available);
}

#[test]
fn step_respects_demand_failure_and_explicit_retry() {
    let mut d = definition();
    d.nodes.get_mut("first").unwrap().activation = Activation::OnDemand;
    let graph = Arc::new(registry().compile(d).unwrap());
    let mut e = Engine::new([graph.clone()]).unwrap();
    begin(&mut e, &graph, "run", 1);
    pause(&mut e, "run");
    assert!(e.apply(Event::Step { run: "run".into() }).is_err());
    e.apply(demand("run", "first")).unwrap();
    step(&mut e, "run");
    let work = advance(&mut e, capacity(1));
    e.apply(Event::Settle {
        execution: work[0].execution,
        outcome: Err(unclassified("fixture", "node")),
    })
    .unwrap();
    assert!(e.inspect("run").unwrap().stepping.is_none());
    assert!(!e.inspect("run").unwrap().step_available);
    e.apply(Event::Retry {
        run: "run".into(),
        node: "first".into(),
    })
    .unwrap();
    step(&mut e, "run");
    assert_eq!(advance(&mut e, capacity(1)).len(), 1);
}

#[test]
fn step_recovery_revokes_permission_without_replaying_work() {
    for dispatched in [false, true] {
        let graph = Arc::new(registry().compile(definition()).unwrap());
        let mut e = Engine::new([graph.clone()]).unwrap();
        begin(&mut e, &graph, "run", 1);
        pause(&mut e, "run");
        step(&mut e, "run");
        let work = e.apply(capacity(1)).unwrap();
        if dispatched {
            e.claim(work[0].execution).unwrap();
        }
        e.apply(Event::Recover).unwrap();
        assert!(e.inspect("run").unwrap().stepping.is_none());
        assert!(advance(&mut e, capacity(1)).is_empty());
        assert_eq!(
            e.disposition("run", "first").unwrap(),
            if dispatched {
                Disposition::Unknown
            } else {
                Disposition::Failed
            }
        );
    }
}

#[test]
fn step_uses_shared_results_without_another_execution_and_blocks_running_steps() {
    let graph = Arc::new(registry().compile(definition()).unwrap());
    let mut e = Engine::new([graph.clone()]).unwrap();
    begin(&mut e, &graph, "a", 1);
    let work = advance(&mut e, capacity(1));
    pause(&mut e, "a");
    assert!(e.apply(Event::Step { run: "a".into() }).is_err());
    e.apply(Event::Settle {
        execution: work[0].execution,
        outcome: Ok(values(2)),
    })
    .unwrap();
    step(&mut e, "a");
    adopt(&mut e, "a", "first");
    begin(&mut e, &graph, "b", 1);
    pause(&mut e, "b");
    step(&mut e, "b");
    assert!(advance(&mut e, capacity(0)).is_empty());
    assert_eq!(e.disposition("b", "first").unwrap(), Disposition::Available);
    adopt(&mut e, "b", "first");
    assert_eq!(e.disposition("b", "second").unwrap(), Disposition::Paused);
}
