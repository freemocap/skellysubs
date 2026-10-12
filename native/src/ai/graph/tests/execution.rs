use super::*;

#[tokio::test]
async fn actual_handler_execution_requires_adoption_before_downstream_dispatch() {
    let graph = Arc::new(registry().compile(definition()).unwrap());
    let mut engine = Engine::new([graph.clone()]).unwrap();
    begin(&mut engine, &graph, "run", 40);
    let work = engine.apply(capacity(1)).unwrap();
    assert_eq!(work.len(), 1);
    let result = engine
        .claim(work[0].execution)
        .unwrap()
        .execute(evidence_limits())
        .await
        .outcome
        .unwrap();
    assert_eq!(result, values(41));
    engine
        .apply(Event::Settle {
            execution: work[0].execution,
            outcome: Ok(result),
        })
        .unwrap();
    assert!(engine.apply(capacity(1)).unwrap().is_empty());
    assert!(engine.outputs("run").unwrap().is_none());
    adopt(&mut engine, "run", "first");
    let work = engine.apply(capacity(1)).unwrap();
    let result = engine
        .claim(work[0].execution)
        .unwrap()
        .execute(evidence_limits())
        .await
        .outcome
        .unwrap();
    engine
        .apply(Event::Settle {
            execution: work[0].execution,
            outcome: Ok(result),
        })
        .unwrap();
    adopt(&mut engine, "run", "second");
    assert_eq!(engine.outputs("run").unwrap(), Some(values(42)));
    assert!(engine.apply(capacity(1)).unwrap().is_empty());
    assert_eq!(
        engine.inspect("run").unwrap().artifact.definition,
        definition()
    );
}

#[test]
fn dormant_disabled_and_guarded_nodes_remain_in_the_artifact() {
    let mut d = definition();
    d.nodes.get_mut("first").unwrap().activation = Activation::OnDemand;
    d.nodes.get_mut("second").unwrap().guard = Some(Source::Constant {
        contract: contract("boolean"),
        value: json!(false),
    });
    d.outputs.get_mut("value").unwrap().optional = true;
    let graph = Arc::new(registry().compile(d.clone()).unwrap());
    let mut engine = Engine::new([graph.clone()]).unwrap();
    begin(&mut engine, &graph, "run", 0);
    assert_eq!(
        engine.disposition("run", "first").unwrap(),
        Disposition::Unrequested
    );
    assert_eq!(
        engine.disposition("run", "second").unwrap(),
        Disposition::Skipped
    );
    assert!(advance(&mut engine, capacity(2)).is_empty());
    engine.apply(demand("run", "first")).unwrap();
    assert_eq!(advance(&mut engine, capacity(2)).len(), 1);
    assert_eq!(engine.inspect("run").unwrap().artifact.definition, d);
}

#[test]
fn sharing_is_across_consumers_not_nodes_and_needs_no_second_slot() {
    let graph = Arc::new(registry().compile(definition()).unwrap());
    let mut engine = Engine::new([graph.clone()]).unwrap();
    begin(&mut engine, &graph, "a", 1);
    begin(&mut engine, &graph, "b", 1);
    let work = advance(&mut engine, capacity(1));
    assert_eq!(work.len(), 1);
    let a = engine.inspect("a").unwrap().attempts["first"][0].clone();
    let b = engine.inspect("b").unwrap().attempts["first"][0].clone();
    assert_ne!(a.id, b.id);
    assert_eq!(a.execution, b.execution);
    engine
        .apply(Event::Cancel {
            run: "a".into(),
            node: None,
        })
        .unwrap();
    engine
        .apply(Event::Settle {
            execution: work[0].execution,
            outcome: Ok(values(2)),
        })
        .unwrap();
    assert_eq!(
        engine.disposition("a", "first").unwrap(),
        Disposition::Cancelled
    );
    adopt(&mut engine, "b", "first");
    assert!(
        engine
            .apply(Event::Adopt {
                run: "a".into(),
                node: "first".into(),
                attempt: a.id
            })
            .is_err()
    );
    begin(&mut engine, &graph, "c", 1);
    assert!(advance(&mut engine, capacity(0)).is_empty());
    assert_eq!(
        engine.disposition("c", "first").unwrap(),
        Disposition::Available
    );
    assert_eq!(
        engine.inspect("c").unwrap().attempts["first"][0].execution,
        a.execution
    );
}

#[test]
fn invalid_output_blocks_children_and_retry_obeys_pause_and_capacity() {
    let graph = Arc::new(registry().compile(definition()).unwrap());
    let mut engine = Engine::new([graph.clone()]).unwrap();
    begin(&mut engine, &graph, "run", 1);
    let first = advance(&mut engine, capacity(1)).remove(0);
    engine
        .apply(Event::Settle {
            execution: first.execution,
            outcome: Ok(BTreeMap::from([(
                "value".into(),
                json!("private invalid content"),
            )])),
        })
        .unwrap();
    assert_eq!(
        engine.disposition("run", "second").unwrap(),
        Disposition::Blocked
    );
    assert!(
        !serde_json::to_string(&engine.inspect("run").unwrap())
            .unwrap()
            .contains("private invalid content")
    );
    engine
        .apply(Event::Pause {
            run: "run".into(),
            paused: true,
        })
        .unwrap();
    engine
        .apply(Event::Retry {
            run: "run".into(),
            node: "first".into(),
        })
        .unwrap();
    assert!(advance(&mut engine, capacity(1)).is_empty());
    engine
        .apply(Event::Pause {
            run: "run".into(),
            paused: false,
        })
        .unwrap();
    assert!(advance(&mut engine, capacity(0)).is_empty());
    assert_eq!(
        engine.inspect("run").unwrap().nodes["first"],
        Disposition::Held
    );
    let retry = advance(&mut engine, capacity(1)).remove(0);
    assert_ne!(retry.execution, first.execution);
    assert_eq!(engine.inspect("run").unwrap().attempts["first"].len(), 2);
}

#[test]
fn replay_never_dispatches_and_unknown_work_never_automatically_replays() {
    let graph = Arc::new(registry().compile(definition()).unwrap());
    let mut engine = Engine::new([graph.clone()]).unwrap();
    begin(&mut engine, &graph, "run", 1);
    advance(&mut engine, capacity(1));
    let json = serde_json::to_string(engine.journal()).unwrap();
    let journal: Vec<Event> = serde_json::from_str(&json).unwrap();
    let mut recovered = Engine::restore([graph.clone()], &journal).unwrap();
    assert_eq!(
        recovered.disposition("run", "first").unwrap(),
        Disposition::Unknown
    );
    assert!(recovered.apply(capacity(5)).unwrap().is_empty());
    let mut incompatible = definition();
    incompatible.contract.version = 2;
    let other = Arc::new(registry().compile(incompatible).unwrap());
    assert_eq!(
        Engine::restore([other], &journal).err().unwrap().code,
        "unknown_artifact"
    );
    let again = Engine::restore([graph], recovered.journal()).unwrap();
    assert_eq!(
        again.disposition("run", "first").unwrap(),
        Disposition::Unknown
    );
}

#[test]
fn rejected_events_are_atomic_and_adoption_cannot_be_duplicated() {
    let graph = Arc::new(registry().compile(definition()).unwrap());
    let mut engine = Engine::new([graph.clone()]).unwrap();
    begin(&mut engine, &graph, "run", 1);
    let before = engine.journal().to_vec();
    assert!(engine.apply(demand("run", "missing")).is_err());
    assert_eq!(engine.journal(), before);
    let work = advance(&mut engine, capacity(1)).remove(0);
    engine
        .apply(Event::Settle {
            execution: work.execution,
            outcome: Ok(values(2)),
        })
        .unwrap();
    adopt(&mut engine, "run", "first");
    let id = engine.inspect("run").unwrap().attempts["first"][0].id;
    let before = engine.journal().to_vec();
    assert!(
        engine
            .apply(Event::Adopt {
                run: "run".into(),
                node: "first".into(),
                attempt: id
            })
            .is_err()
    );
    assert_eq!(engine.journal(), before);
}
