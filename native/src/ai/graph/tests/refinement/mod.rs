use super::*;
use std::collections::VecDeque;

mod model;
use model::{ACTIONS, Action, Consumer, Failure, Producer, State};

const RUNS: [&str; 2] = ["a", "b"];

fn failure(fault: &Fault) -> Failure {
    match (fault.code.as_str(), fault.path.as_str()) {
        ("fixture_failure", "provider") => Failure::Provider,
        ("cancelled_before_dispatch", "execution") => Failure::Abandoned,
        ("interrupted_before_dispatch", "execution") => Failure::Interrupted,
        other => panic!("unexpected failure {other:?}"),
    }
}

/// Abstraction map only: no scheduling, eligibility or transition decisions.
fn abstract_state(engine: &Engine, execution: ExecutionId) -> State {
    let producer = &engine.state.executions[&execution];
    let state = match &producer.outcome {
        Some(Ok(value)) => {
            assert_eq!(value, &values(2));
            Producer::Success
        }
        Some(Err(error)) => Producer::Failed(failure(error)),
        None if producer.unknown => Producer::Unknown,
        None if producer.dispatched => Producer::Running,
        None => Producer::Queued,
    };
    State {
        producer: state,
        consumers: RUNS.map(|run| {
            assert_eq!(engine.state.run_attempts(run).count(), 1);
            let attempt = &engine.state.attempts[&engine.state.runs[run].current["first"]].attempt;
            assert_eq!(attempt.execution, execution);
            match &attempt.state {
                AttemptState::Prepared => Consumer::Queued,
                AttemptState::Running => Consumer::Running,
                AttemptState::Available => Consumer::Available,
                AttemptState::Adopted => Consumer::Adopted,
                AttemptState::Failed(error) => Consumer::Failed(failure(error)),
                AttemptState::Unknown => Consumer::Unknown,
                AttemptState::Cancelled => Consumer::Cancelled,
            }
        }),
        paused: RUNS.map(|run| engine.state.runs[run].paused),
        active: RUNS.map(|run| engine.state.runs[run].active),
        capacity: engine.state.capacity.provider == 1,
    }
}

fn event(action: Action, engine: &Engine, execution: ExecutionId) -> Event {
    match action {
        Action::Pause(i, paused) => Event::Pause {
            run: RUNS[i].into(),
            paused,
        },
        Action::Cancel(i) => Event::Cancel {
            run: RUNS[i].into(),
            node: None,
        },
        Action::Adopt(i) => Event::Adopt {
            run: RUNS[i].into(),
            node: "first".into(),
            attempt: engine.state.runs[RUNS[i]].current["first"],
        },
        Action::Advance(available) => capacity(usize::from(available)),
        Action::Dispatch => Event::Dispatch { execution },
        Action::Settle(success) => Event::Settle {
            execution,
            outcome: if success {
                Ok(values(2))
            } else {
                Err(unclassified("fixture_failure", "provider"))
            },
        },
        Action::Recover => Event::Recover,
    }
}

fn check_observations(engine: &Engine, graph: &Executable, state: State) {
    super::execution_queries::check(engine);
    assert_eq!(engine.state.executions.len(), 1);
    assert_eq!(engine.state.runs.len(), 2);
    for (i, run) in RUNS.into_iter().enumerate() {
        let view = engine.inspect(run).unwrap();
        assert_eq!(view.artifact, graph.artifact());
        assert_eq!(view.nodes.len(), 2);
        assert_eq!(view.artifact_id, graph.identity());
        let expected = match state.consumers[i] {
            Consumer::Queued => Disposition::Prepared,
            Consumer::Running => Disposition::Running,
            Consumer::Available => Disposition::Available,
            Consumer::Adopted => Disposition::Adopted,
            Consumer::Failed(_) => Disposition::Failed,
            Consumer::Unknown => Disposition::Unknown,
            Consumer::Cancelled => Disposition::Cancelled,
        };
        assert_eq!(view.nodes["first"], expected);
        assert_eq!(
            view.nodes["second"],
            if state.active[i] {
                Disposition::Disabled
            } else {
                Disposition::Cancelled
            }
        );
        assert!(!view.attempts.contains_key("second"));
        assert_eq!(
            engine.outputs(run).unwrap(),
            (state.consumers[i] == Consumer::Adopted).then(|| values(2))
        );
    }
}

#[test]
fn shared_producer_refines_finite_lifecycle_specification() {
    let mut definition = definition();
    definition.nodes.get_mut("second").unwrap().activation = Activation::Disabled;
    definition.results.insert("value".into(), source("first"));
    let graph = Arc::new(registry().compile(definition).unwrap());
    let mut engine = Engine::new([graph.clone()]).unwrap();
    for run in RUNS {
        begin(&mut engine, &graph, run, 1);
    }
    let work = engine.apply(capacity(1)).unwrap();
    assert_eq!(work.len(), 1);
    let work = work[0].clone();
    let execution = work.execution;
    assert_eq!(abstract_state(&engine, execution), State::INITIAL);

    // One representative per abstract state; require complete concrete snapshot
    // equality on every merge, so hidden mutable fields cannot be discarded.
    // Journal/revision are intentionally excluded from this finite quotient.
    let mut seen = BTreeMap::from([(State::INITIAL, engine.state.clone())]);
    let mut queue = VecDeque::from([(State::INITIAL, engine, Vec::<Action>::new())]);
    let mut accepted = 0;
    let mut rejected = 0;
    while let Some((state, engine, trace)) = queue.pop_front() {
        check_observations(&engine, &graph, state);
        for action in ACTIONS {
            let mut path = trace.clone();
            path.push(action);
            let expected = state.step(action);
            let mut candidate = engine.clone();
            let actual = candidate.apply(event(action, &engine, execution));
            assert_eq!(actual.is_ok(), expected.is_some(), "trace: {path:?}");
            if let Some((next, work_count)) = expected {
                accepted += 1;
                assert_eq!(actual.unwrap(), vec![work.clone(); work_count], "{path:?}");
                assert_eq!(abstract_state(&candidate, execution), next, "{path:?}");
                assert_eq!(candidate.revision(), engine.revision() + 1);
                check_observations(&candidate, &graph, next);
                let snapshot = candidate.state.clone();
                if let Some(previous) = seen.get(&next) {
                    assert_eq!(previous, &snapshot, "hidden-state collision: {path:?}");
                } else {
                    seen.insert(next, snapshot);
                    queue.push_back((next, candidate, path));
                }
            } else {
                rejected += 1;
                assert_eq!(candidate.state.clone(), engine.state.clone());
                assert_eq!(candidate.journal(), engine.journal());
                assert_eq!(candidate.revision(), engine.revision());
            }
        }
    }
    assert!(
        seen.keys()
            .any(|s| s.consumers == [Consumer::Cancelled, Consumer::Adopted])
    );
    assert!(seen.keys().any(|s| s.consumers == [Consumer::Adopted; 2]));
    assert!(seen.keys().any(|s| s.producer == Producer::Unknown));
    assert!(
        seen.keys()
            .any(|s| s.producer == Producer::Failed(Failure::Abandoned))
    );
    assert!(
        seen.keys()
            .any(|s| s.producer == Producer::Failed(Failure::Interrupted))
    );
    assert_eq!(accepted + rejected, seen.len() * ACTIONS.len());
    // Reviewed state-space size: catch accidental removal of actions or branches.
    assert_eq!((seen.len(), accepted, rejected), (400, 3735, 1865));
    println!(
        "finite lifecycle: {} states, {accepted} accepted edges, {rejected} rejected edges",
        seen.len()
    );
}
