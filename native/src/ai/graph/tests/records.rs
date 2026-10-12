use super::*;
use crate::ai::graph::state::RuntimeState;

fn prepared() -> (Engine, Vec<Work>) {
    let mut definition = definition();
    definition.nodes.get_mut("second").unwrap().activation = Activation::Disabled;
    definition.results.insert("value".into(), source("first"));
    let graph = Arc::new(registry().compile(definition).unwrap());
    let mut engine = Engine::new([graph.clone()]).unwrap();
    for (run, input) in [("a", 1), ("b", 1), ("unrelated", 2)] {
        begin(&mut engine, &graph, run, input);
    }
    let work = engine.apply(capacity(2)).unwrap();
    assert_eq!(work.len(), 2);
    (engine, work)
}

fn assert_shared(before: &RuntimeState, after: &RuntimeState, run: &str, execution: ExecutionId) {
    assert!(std::ptr::eq(&before.runs[run], &after.runs[run]));
    assert!(std::ptr::eq(
        &before.executions[&execution],
        &after.executions[&execution]
    ));
}

#[test]
fn transactions_detach_affected_records_without_copying_unrelated_payloads() {
    let (mut engine, work) = prepared();
    let execution = work[0].execution;
    let unrelated = work[1].execution;
    let prepared = engine.state.clone();
    let prepared_bytes = serde_json::to_vec(&prepared).unwrap();
    for run in ["a", "b", "unrelated"] {
        assert!(std::ptr::eq(&prepared.runs[run], &engine.state.runs[run]));
    }
    engine.apply(Event::Dispatch { execution }).unwrap();
    assert_shared(&prepared, &engine.state, "unrelated", unrelated);
    for run in ["a", "b"] {
        assert!(std::ptr::eq(&prepared.runs[run], &engine.state.runs[run]));
        let id = prepared.runs[run].current["first"];
        assert!(!std::ptr::eq(
            &prepared.attempts[&id],
            &engine.state.attempts[&id]
        ));
        assert_eq!(prepared.attempts[&id].attempt.state, AttemptState::Prepared);
        assert_eq!(
            engine.disposition(run, "first").unwrap(),
            Disposition::Running
        );
    }
    assert!(!prepared.executions[&execution].dispatched);

    let running = engine.state.clone();
    engine
        .apply(Event::Cancel {
            run: "a".into(),
            node: None,
        })
        .unwrap();
    assert_shared(&running, &engine.state, "b", execution);
    assert_shared(&running, &engine.state, "unrelated", unrelated);

    let cancelled = engine.state.clone();
    engine
        .apply(Event::Settle {
            execution,
            outcome: Ok(values(2)),
        })
        .unwrap();
    // Settlement changes only the surviving subscriber and producing execution.
    assert!(std::ptr::eq(&cancelled.runs["a"], &engine.state.runs["a"]));
    assert!(std::ptr::eq(&cancelled.runs["b"], &engine.state.runs["b"]));
    let a = cancelled.runs["a"].current["first"];
    let b = cancelled.runs["b"].current["first"];
    assert!(std::ptr::eq(
        &cancelled.attempts[&a],
        &engine.state.attempts[&a]
    ));
    assert!(!std::ptr::eq(
        &cancelled.attempts[&b],
        &engine.state.attempts[&b]
    ));
    assert_shared(&cancelled, &engine.state, "unrelated", unrelated);
    assert!(cancelled.executions[&execution].outcome.is_none());

    let available = engine.state.clone();
    adopt(&mut engine, "b", "first");
    assert_shared(&available, &engine.state, "unrelated", unrelated);
    assert_shared(&available, &engine.state, "a", execution);
    assert_eq!(serde_json::to_vec(&prepared).unwrap(), prepared_bytes);
}

#[test]
fn canonical_state_serializes_independent_attempt_records() {
    let (engine, _) = prepared();
    let runs: BTreeMap<_, _> = engine.state.runs.iter().collect();
    let executions: BTreeMap<_, _> = engine.state.executions.iter().collect();
    let attempts: BTreeMap<_, _> = engine.state.attempts.iter().collect();
    // Fixed physical field names; record ownership adds no serialization envelope.
    let expected = json!({
        "runs": runs,
        "attempts": attempts,
        "executions": executions,
        "next_id": engine.state.next_id,
        "capacity": engine.state.capacity,
        "held": engine.state.held,
    });
    let encoded = serde_json::to_value(&engine.state).unwrap();
    assert_eq!(encoded, expected);
    let mut decoded: RuntimeState = serde_json::from_value(encoded.clone()).unwrap();
    assert_eq!(decoded, engine.state);
    decoded.runs.get_mut("a").unwrap().paused = true;
    assert!(!engine.state.runs["a"].paused);
    let mut unknown = encoded;
    unknown["unexpected"] = json!(true);
    assert!(serde_json::from_value::<RuntimeState>(unknown).is_err());
}

#[test]
fn repeated_recovery_and_rejected_events_keep_record_ownership_intact() {
    let (mut engine, work) = prepared();
    engine
        .apply(Event::Dispatch {
            execution: work[0].execution,
        })
        .unwrap();
    engine.apply(Event::Recover).unwrap();
    let recovered = engine.state.clone();
    let revision = engine.revision();
    engine.apply(Event::Recover).unwrap();
    assert_eq!(engine.state, recovered);
    assert_eq!(engine.revision(), revision + 1);
    assert_shared(&recovered, &engine.state, "a", work[0].execution);
    assert_shared(&recovered, &engine.state, "b", work[0].execution);
    assert_shared(&recovered, &engine.state, "unrelated", work[1].execution);

    let journal = engine.journal().to_vec();
    assert!(
        engine
            .apply(Event::Dispatch {
                execution: work[0].execution
            })
            .is_err()
    );
    assert_eq!(engine.journal(), journal);
    assert_shared(&recovered, &engine.state, "a", work[0].execution);
    assert_shared(&recovered, &engine.state, "unrelated", work[1].execution);
}
