use super::*;
use crate::ai::graph::state::RuntimeState;
use std::collections::BTreeSet;

/// Scan primary records independently of every derived index. This is a test
/// oracle for query completeness, ordering and filtering, not a second runtime.
pub(super) fn check(engine: &Engine) {
    let state = &engine.state;
    let unresolved: Vec<_> = state
        .executions
        .iter()
        .filter(|(_, row)| row.outcome.is_none() && !row.unknown)
        .map(|(id, _)| *id)
        .collect();
    assert_eq!(
        state
            .executions
            .unresolved()
            .map(|(id, _)| id)
            .collect::<Vec<_>>(),
        unresolved
    );
    for (execution, row) in state.executions.iter() {
        let expected: Vec<_> = state
            .executions
            .iter()
            .filter(|(_, candidate)| candidate.key == row.key)
            .map(|(id, _)| *id)
            .collect();
        assert_eq!(
            state
                .executions
                .with_key(&row.key)
                .map(|(id, _)| id)
                .collect::<Vec<_>>(),
            expected
        );
        let expected: BTreeSet<_> = state
            .runs
            .iter()
            .flat_map(|(run_id, run)| {
                run.current.iter().filter_map(|(node, id)| {
                    let a = &state.attempts[id].attempt;
                    (a.execution == *execution).then_some((run_id.clone(), node.clone(), *id))
                })
            })
            .collect();
        let actual = consumers(engine, *execution);
        assert_eq!(actual, expected);
        for allow_paused in [false, true] {
            let eligible = expected.iter().any(|(run, node, id)| {
                let r = &state.runs[run];
                r.active
                    && (allow_paused || !r.paused)
                    && !r.cancelled.contains(node)
                    && matches!(
                        state.attempts[id].attempt.state,
                        AttemptState::Prepared | AttemptState::Running
                    )
            });
            assert_eq!(
                engine
                    .has_consumer_using(&mut &engine.state, *execution, allow_paused)
                    .unwrap(),
                eligible
            );
        }
    }
    assert_eq!(state.executions.with_key("missing-key").count(), 0);
    assert!(consumers(engine, ExecutionId(u64::MAX)).is_empty());
}

fn consumers(engine: &Engine, execution: ExecutionId) -> BTreeSet<(String, String, AttemptId)> {
    let mut rows = BTreeSet::new();
    engine
        .visit_consumers(&mut &engine.state, execution, |_, row| {
            rows.insert((row.run.clone(), row.node.clone(), row.attempt.id));
        })
        .unwrap();
    rows
}

fn apply(engine: &mut Engine, event: Event) -> Vec<Work> {
    let work = engine.apply(event).unwrap();
    check(engine);
    let bytes = serde_json::to_vec(&engine.state).unwrap();
    let decoded: RuntimeState = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(decoded, engine.state);
    assert_eq!(serde_json::to_vec(&decoded).unwrap(), bytes);
    work
}

fn setup() -> (Arc<Executable>, Engine) {
    let mut d = definition();
    d.nodes.get_mut("second").unwrap().activation = Activation::Disabled;
    d.results.insert("value".into(), source("first"));
    let graph = Arc::new(registry().compile(d).unwrap());
    let mut engine = Engine::new([graph.clone()]).unwrap();
    for (run, input) in [("a", 1), ("b", 1), ("unrelated", 9)] {
        begin(&mut engine, &graph, run, input);
    }
    (graph, engine)
}

fn current(engine: &Engine, run: &str) -> Attempt {
    engine.state.attempts[&engine.state.runs[run].current["first"]]
        .attempt
        .clone()
}

#[test]
fn indexed_reuse_and_consumers_preserve_retry_history_and_oldest_success() {
    let (graph, mut engine) = setup();
    let work = apply(&mut engine, capacity(3));
    assert_eq!(work.len(), 2);
    let first = current(&engine, "a");
    assert_eq!(first.execution, current(&engine, "b").execution);
    apply(
        &mut engine,
        Event::Dispatch {
            execution: first.execution,
        },
    );
    apply(
        &mut engine,
        Event::Settle {
            execution: first.execution,
            outcome: Err(unclassified("fixture", "provider")),
        },
    );
    apply(
        &mut engine,
        Event::Retry {
            run: "a".into(),
            node: "first".into(),
        },
    );
    apply(&mut engine, capacity(3));
    let second = current(&engine, "a");
    assert_ne!(first.execution, second.execution);
    assert_eq!(consumers(&engine, first.execution).len(), 1);
    apply(
        &mut engine,
        Event::Dispatch {
            execution: second.execution,
        },
    );
    begin(&mut engine, &graph, "c", 1);
    apply(&mut engine, capacity(3));
    assert_eq!(current(&engine, "c").execution, second.execution);
    let frozen = engine.state.clone();
    apply(
        &mut engine,
        Event::Cancel {
            run: "a".into(),
            node: None,
        },
    );
    apply(
        &mut engine,
        Event::Settle {
            execution: second.execution,
            outcome: Ok(values(2)),
        },
    );
    assert_eq!(current(&engine, "a").state, AttemptState::Cancelled);
    assert_eq!(current(&engine, "c").state, AttemptState::Available);
    assert!(frozen.executions[&second.execution].outcome.is_none());
    assert!(matches!(
        engine.state.attempts[&first.id].attempt.state,
        AttemptState::Failed(_)
    ));

    // An explicit retry produces fresh work even with a retained success. A new
    // ordinary consumer still chooses the oldest eligible producer in ID order.
    apply(
        &mut engine,
        Event::Retry {
            run: "b".into(),
            node: "first".into(),
        },
    );
    apply(&mut engine, capacity(3));
    let third = current(&engine, "b");
    assert_ne!(third.execution, second.execution);
    apply(
        &mut engine,
        Event::Dispatch {
            execution: third.execution,
        },
    );
    apply(
        &mut engine,
        Event::Settle {
            execution: third.execution,
            outcome: Ok(values(2)),
        },
    );
    begin(&mut engine, &graph, "retained", 1);
    apply(&mut engine, capacity(3));
    assert_eq!(current(&engine, "retained").execution, second.execution);
    assert_eq!(
        current(&engine, "retained").acquisition,
        Acquisition::Retained
    );
    assert_eq!(consumers(&engine, first.execution).len(), 0);
    assert_eq!(
        engine.state.attempts.for_execution(first.execution).count(),
        2
    );
    apply(&mut engine, Event::Recover);
    check(&Engine::restore([graph], engine.journal()).unwrap());
}

#[test]
fn recovery_and_pause_keep_query_membership_separate_from_eligibility() {
    let (graph, mut engine) = setup();
    apply(&mut engine, capacity(3));
    let first = current(&engine, "a");
    for run in ["a", "b"] {
        apply(
            &mut engine,
            Event::Pause {
                run: run.into(),
                paused: true,
            },
        );
    }
    assert!(
        !engine
            .has_consumer_using(&mut &engine.state, first.execution, false)
            .unwrap()
    );
    assert!(
        engine
            .has_consumer_using(&mut &engine.state, first.execution, true)
            .unwrap()
    );
    apply(
        &mut engine,
        Event::Pause {
            run: "a".into(),
            paused: false,
        },
    );
    apply(
        &mut engine,
        Event::Dispatch {
            execution: first.execution,
        },
    );
    apply(&mut engine, Event::Recover);
    assert!(engine.state.executions[&first.execution].unknown);
    begin(&mut engine, &graph, "new", 1);
    apply(&mut engine, capacity(3));
    assert_ne!(current(&engine, "new").execution, first.execution);
    assert_eq!(consumers(&engine, first.execution).len(), 2);
    assert!(
        !engine
            .has_consumer_using(&mut &engine.state, first.execution, true)
            .unwrap()
    );
}

#[test]
fn unresolved_index_preserves_running_occupancy_until_settlement_or_recovery() {
    let (_, mut engine) = setup();
    let work = apply(&mut engine, capacity(1));
    assert_eq!(work.len(), 1);
    let execution = work[0].execution;
    apply(&mut engine, Event::Dispatch { execution });
    let running = engine.state.clone();
    for run in ["a", "b"] {
        apply(
            &mut engine,
            Event::Cancel {
                run: run.into(),
                node: None,
            },
        );
    }
    // Revoking every consumer does not free an already dispatched provider slot.
    assert_eq!(engine.state.executions.unresolved().len(), 1);
    assert!(apply(&mut engine, capacity(1)).is_empty());
    let before = engine.state.clone();
    let journal = engine.journal().to_vec();
    assert!(engine.apply(Event::Dispatch { execution }).is_err());
    assert_eq!(engine.state, before);
    assert_eq!(engine.journal(), journal);
    // Malformed success becomes a terminal validation failure and releases it.
    apply(
        &mut engine,
        Event::Settle {
            execution,
            outcome: Ok(Values::new()),
        },
    );
    assert_eq!(engine.state.executions.unresolved().len(), 0);
    assert!(matches!(
        engine.state.executions[&execution].outcome,
        Some(Err(_))
    ));
    assert_eq!(running.executions.unresolved().len(), 1);
    assert!(running.executions[&execution].outcome.is_none());
    let work = apply(&mut engine, capacity(1));
    assert_eq!(work.len(), 1);
    assert_ne!(work[0].execution, execution);
    assert_eq!(engine.state.executions.len(), 2);
    assert_eq!(engine.state.executions.unresolved().len(), 1);
    // Recovery closes queued work; history remains independently queryable.
    apply(&mut engine, Event::Recover);
    assert_eq!(engine.state.executions.unresolved().len(), 0);
    assert_eq!(engine.state.executions.len(), 2);
    let recovered = engine.state.clone();
    apply(&mut engine, Event::Recover);
    assert_eq!(engine.state, recovered);
}
