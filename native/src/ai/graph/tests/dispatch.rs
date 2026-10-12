use super::*;

#[test]
fn new_subscriber_releases_prepared_work_from_a_paused_owner() {
    let graph = Arc::new(registry().compile(definition()).unwrap());
    let mut engine = Engine::new([graph.clone()]).unwrap();
    begin(&mut engine, &graph, "a", 1);
    let prepared = engine.apply(capacity(1)).unwrap().remove(0);
    engine
        .apply(Event::Pause {
            run: "a".into(),
            paused: true,
        })
        .unwrap();
    begin(&mut engine, &graph, "b", 1);
    let work = engine.apply(capacity(1)).unwrap();
    assert_eq!(work, vec![prepared.clone()]);
    assert_eq!(
        engine.inspect("b").unwrap().attempts["first"][0].acquisition,
        Acquisition::Subscribed
    );
    assert!(engine.claim(prepared.execution).is_ok());
}

#[tokio::test]
async fn claim_uses_captured_inputs_and_cannot_be_repeated() {
    let graph = Arc::new(registry().compile(definition()).unwrap());
    let mut engine = Engine::new([graph.clone()]).unwrap();
    begin(&mut engine, &graph, "run", 40);
    let mut descriptor = engine.apply(capacity(1)).unwrap().remove(0);
    descriptor.inputs = values(900);
    let invocation = engine.claim(descriptor.execution).unwrap();
    assert!(engine.claim(descriptor.execution).is_err());
    let output = invocation.execute(evidence_limits()).await.outcome.unwrap();
    assert_eq!(output, values(41));
    engine
        .apply(Event::Settle {
            execution: descriptor.execution,
            outcome: Ok(output.clone()),
        })
        .unwrap();
    let attempt = engine.inspect("run").unwrap().attempts["first"][0].id;
    assert_eq!(engine.available("run", "first", attempt).unwrap(), output);
    assert_eq!(
        engine.disposition("run", "second").unwrap(),
        Disposition::Waiting
    );
    adopt(&mut engine, "run", "first");
    assert!(engine.available("run", "first", attempt).is_err());
}

#[test]
fn cancelling_unclaimed_work_prevents_dispatch_and_reuse() {
    let graph = Arc::new(registry().compile(definition()).unwrap());
    let mut engine = Engine::new([graph.clone()]).unwrap();
    begin(&mut engine, &graph, "a", 1);
    let old = engine.apply(capacity(1)).unwrap().remove(0);
    engine
        .apply(Event::Cancel {
            run: "a".into(),
            node: None,
        })
        .unwrap();
    assert!(engine.claim(old.execution).is_err());
    begin(&mut engine, &graph, "b", 1);
    let new = engine.apply(capacity(1)).unwrap().remove(0);
    assert_ne!(old.execution, new.execution);
    assert!(engine.claim(new.execution).is_ok());
}

#[test]
fn claim_rechecks_pause_and_capacity_and_recovery_distinguishes_dispatch() {
    let graph = Arc::new(registry().compile(definition()).unwrap());
    let mut engine = Engine::new([graph.clone()]).unwrap();
    begin(&mut engine, &graph, "a", 1);
    begin(&mut engine, &graph, "b", 10);
    let work = engine.apply(capacity(2)).unwrap();
    engine
        .apply(Event::Pause {
            run: "a".into(),
            paused: true,
        })
        .unwrap();
    assert!(engine.claim(work[0].execution).is_err());
    engine
        .apply(Event::Pause {
            run: "a".into(),
            paused: false,
        })
        .unwrap();
    engine.apply(capacity(1)).unwrap();
    let _claimed = engine.claim(work[0].execution).unwrap();
    assert!(engine.claim(work[1].execution).is_err());
    let recovered = Engine::restore([graph], engine.journal()).unwrap();
    assert_eq!(
        recovered.disposition("a", "first").unwrap(),
        Disposition::Unknown
    );
    assert_eq!(
        recovered.disposition("b", "first").unwrap(),
        Disposition::Failed
    );
    let view = recovered.inspect("b").unwrap();
    assert!(
        matches!(&view.attempts["first"][0].state, AttemptState::Failed(f) if f.code == "interrupted_before_dispatch")
    );
}

#[test]
fn native_inspection_explains_dependencies_admission_and_acquisition() {
    let graph = Arc::new(registry().compile(definition()).unwrap());
    let mut engine = Engine::new([graph.clone()]).unwrap();
    begin(&mut engine, &graph, "a", 1);
    begin(&mut engine, &graph, "b", 1);
    engine.apply(capacity(0)).unwrap();
    let view = engine.inspect("a").unwrap();
    assert!(view.reasons["first"].contains(&Reason::Admission(Resource::Provider)));
    assert!(view.reasons["second"].contains(&Reason::Input {
        port: "value".into(),
        producer: Some("first".into()),
        availability: Availability::Waiting
    }));
    let work = advance(&mut engine, capacity(1)).remove(0);
    assert_eq!(
        engine.inspect("a").unwrap().attempts["first"][0].acquisition,
        Acquisition::Produced
    );
    assert_eq!(
        engine.inspect("b").unwrap().attempts["first"][0].acquisition,
        Acquisition::Subscribed
    );
    engine
        .apply(Event::Settle {
            execution: work.execution,
            outcome: Ok(values(2)),
        })
        .unwrap();
    begin(&mut engine, &graph, "c", 1);
    engine.apply(capacity(0)).unwrap();
    assert_eq!(
        engine.inspect("c").unwrap().attempts["first"][0].acquisition,
        Acquisition::Retained
    );
}
