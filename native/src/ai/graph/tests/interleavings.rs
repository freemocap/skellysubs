use super::*;

#[test]
fn fan_in_waits_for_all_adoptions_in_every_independent_completion_order() {
    let mut r = registry();
    let inputs = BTreeMap::from([
        ("a".into(), port(false)),
        ("b".into(), port(false)),
        ("c".into(), port(false)),
    ]);
    r.register(
        Operation {
            contract: contract("sum"),
            implementation: "synthetic/sum/v1".into(),
            inputs,
            outputs: ports(),
            resource: Resource::Local,
            reuse: Reuse::Fresh,
        },
        Arc::new(|_, xs| {
            Box::pin(async move { Ok(values(xs.values().map(|v| v.as_i64().unwrap()).sum())) })
        }),
    )
    .unwrap();
    let mut d = definition();
    d.nodes.clear();
    for (id, n) in [("a", 1), ("b", 2), ("c", 3)] {
        d.nodes.insert(
            id.into(),
            node(Source::Constant {
                contract: contract("integer"),
                value: json!(n),
            }),
        );
    }
    d.nodes.insert(
        "join".into(),
        Node {
            operation: contract("sum"),
            inputs: ["a", "b", "c"]
                .into_iter()
                .map(|n| (n.into(), source(n)))
                .collect(),
            after: vec![],
            guard: None,
            activation: Activation::Automatic,
        },
    );
    d.results.insert("value".into(), source("join"));
    let graph = Arc::new(r.compile(d).unwrap());
    for order in [
        [0, 1, 2],
        [0, 2, 1],
        [1, 0, 2],
        [1, 2, 0],
        [2, 0, 1],
        [2, 1, 0],
    ] {
        let mut engine = Engine::new([graph.clone()]).unwrap();
        begin(&mut engine, &graph, "run", 0);
        let work = advance(&mut engine, capacity(3));
        assert_eq!(work.len(), 3);
        for (i, index) in order.into_iter().enumerate() {
            let w = &work[index];
            engine
                .apply(Event::Settle {
                    execution: w.execution,
                    outcome: Ok(values(w.inputs["value"].as_i64().unwrap() + 1)),
                })
                .unwrap();
            assert!(advance(&mut engine, capacity(3)).is_empty());
            adopt(&mut engine, "run", ["a", "b", "c"][index]);
            if i < 2 {
                assert_eq!(
                    engine.disposition("run", "join").unwrap(),
                    Disposition::Waiting
                );
            }
        }
        // Local execution is independent of provider capacity.
        let join = engine
            .apply(Event::Advance(Capacity {
                local: 1,
                provider: 0,
            }))
            .unwrap();
        assert_eq!(join.len(), 1);
        engine
            .apply(Event::Dispatch {
                execution: join[0].execution,
            })
            .unwrap();
        assert_eq!(
            join[0]
                .inputs
                .values()
                .map(|v| v.as_i64().unwrap())
                .sum::<i64>(),
            9
        );
        engine
            .apply(Event::Settle {
                execution: join[0].execution,
                outcome: Ok(values(9)),
            })
            .unwrap();
        adopt(&mut engine, "run", "join");
        assert_eq!(engine.outputs("run").unwrap(), Some(values(9)));
        assert!(std::ptr::eq(
            engine.inspect("run").unwrap().artifact,
            graph.artifact()
        ));
    }
}

#[test]
fn all_cancellation_and_settlement_orders_preserve_other_consumers() {
    let graph = Arc::new(registry().compile(definition()).unwrap());
    for order in [
        [0, 1, 2],
        [0, 2, 1],
        [1, 0, 2],
        [1, 2, 0],
        [2, 0, 1],
        [2, 1, 0],
    ] {
        let mut engine = Engine::new([graph.clone()]).unwrap();
        for run in ["a", "b", "survivor"] {
            begin(&mut engine, &graph, run, 1);
        }
        let work = advance(&mut engine, capacity(1));
        assert_eq!(work.len(), 1);
        for event in order {
            let event = match event {
                0 => Event::Cancel {
                    run: "a".into(),
                    node: None,
                },
                1 => Event::Cancel {
                    run: "b".into(),
                    node: None,
                },
                _ => Event::Settle {
                    execution: work[0].execution,
                    outcome: Ok(values(2)),
                },
            };
            engine.apply(event).unwrap();
        }
        assert_eq!(
            engine.disposition("a", "first").unwrap(),
            Disposition::Cancelled
        );
        assert_eq!(
            engine.disposition("b", "first").unwrap(),
            Disposition::Cancelled
        );
        assert_eq!(
            engine.disposition("survivor", "first").unwrap(),
            Disposition::Available
        );
        adopt(&mut engine, "survivor", "first");
        assert_eq!(engine.state.executions.len(), 1);
    }
}

#[test]
fn policy_changes_overlays_only_and_disabled_inputs_do_not_become_success() {
    let graph = Arc::new(registry().compile(definition()).unwrap());
    let mut engine = Engine::new([graph.clone()]).unwrap();
    for (run, activation) in [
        ("auto", Activation::Automatic),
        ("demand", Activation::OnDemand),
        ("disabled", Activation::Disabled),
    ] {
        engine
            .apply(Event::Begin {
                run: run.into(),
                artifact: graph.identity().into(),
                inputs: values(0),
                scope: "scope".into(),
                policy: BTreeMap::from([("first".into(), activation)]),
            })
            .unwrap();
        assert!(std::ptr::eq(
            engine.inspect(run).unwrap().artifact,
            graph.artifact()
        ));
        assert_eq!(engine.inspect(run).unwrap().activation["first"], activation);
    }
    assert_eq!(
        engine.disposition("disabled", "second").unwrap(),
        Disposition::Blocked
    );
    assert!(engine.outputs("disabled").unwrap().is_none());
    assert_eq!(
        engine.disposition("demand", "first").unwrap(),
        Disposition::Unrequested
    );
    assert_eq!(advance(&mut engine, capacity(1)).len(), 1);
    engine.apply(demand("demand", "first")).unwrap();
    assert!(advance(&mut engine, capacity(0)).is_empty());
    assert_eq!(
        engine.disposition("demand", "first").unwrap(),
        Disposition::Running
    );
}

#[test]
fn sharing_crosses_artifacts_but_never_authority_scope_or_fresh_policy() {
    let registry = registry();
    let first = Arc::new(registry.compile(definition()).unwrap());
    let mut other = definition();
    other.contract = contract("another-graph");
    let second = Arc::new(registry.compile(other).unwrap());
    let mut engine = Engine::new([first.clone(), second.clone()]).unwrap();
    begin(&mut engine, &first, "a", 4);
    begin(&mut engine, &second, "b", 4);
    engine
        .apply(Event::Begin {
            run: "different-authority".into(),
            artifact: first.identity().into(),
            inputs: values(4),
            scope: "different".into(),
            policy: BTreeMap::new(),
        })
        .unwrap();
    assert_eq!(advance(&mut engine, capacity(2)).len(), 2);
    assert_eq!(
        engine.inspect("a").unwrap().attempts["first"][0].execution,
        engine.inspect("b").unwrap().attempts["first"][0].execution
    );
    let mut r = registry;
    r.operations
        .get_mut(&contract("increment"))
        .unwrap()
        .0
        .reuse = Reuse::Fresh;
    let fresh = Arc::new(r.compile(definition()).unwrap());
    let mut engine = Engine::new([fresh.clone()]).unwrap();
    begin(&mut engine, &fresh, "a", 4);
    begin(&mut engine, &fresh, "b", 4);
    assert_eq!(advance(&mut engine, capacity(2)).len(), 2);
}

#[test]
fn composed_handler_and_member_changes_cannot_silently_rebind_a_subgraph() {
    let mut r = registry();
    let child = r.compile(definition()).unwrap();
    let mut parent = definition();
    parent.nodes.clear();
    parent.results = parent
        .compose(
            "child",
            &child,
            BTreeMap::from([("value".into(), Source::Input("value".into()))]),
        )
        .unwrap();
    let mut changed = parent.clone();
    changed
        .compositions
        .get_mut("child")
        .unwrap()
        .source
        .nodes
        .get_mut("second")
        .unwrap()
        .inputs
        .insert("value".into(), Source::Input("value".into()));
    assert_eq!(
        r.compile(changed).err().unwrap().code,
        "composition_changed"
    );
    r.operations
        .get_mut(&contract("increment"))
        .unwrap()
        .0
        .implementation = "different/v2".into();
    assert_eq!(r.compile(parent).err().unwrap().code, "composition_changed");
}
