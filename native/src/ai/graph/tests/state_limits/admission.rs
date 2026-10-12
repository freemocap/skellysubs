use super::*;

#[test]
fn each_record_limit_rejects_the_whole_event_without_partial_ids_or_work() {
    for (state, scope_b, expected) in [
        (
            StateLimits {
                runs: 1,
                attempts: 2,
                executions: 2,
            },
            "b",
            "run_limit",
        ),
        (
            StateLimits {
                runs: 2,
                attempts: 1,
                executions: 2,
            },
            "shared",
            "attempt_limit",
        ),
        (
            StateLimits {
                runs: 2,
                attempts: 2,
                executions: 1,
            },
            "b",
            "execution_limit",
        ),
    ] {
        let dir = tempfile::tempdir().unwrap();
        let mut store = SqlStore::open(&dir.path().join("owner.db"));
        let graph = graph();
        let mut host = DurableEngine::create([graph.clone()], limits(state), &mut store).unwrap();
        start(&mut host, &mut store, &graph, "a", "shared").unwrap();
        if expected != "run_limit" {
            start(&mut host, &mut store, &graph, "b", scope_b).unwrap();
        }
        let before = store.bytes();
        let view = snapshot(&host, "a");
        let usage = host.state_usage().unwrap();
        let rejected = if expected == "run_limit" {
            start(&mut host, &mut store, &graph, "b", scope_b)
        } else {
            host.apply(capacity(2), &mut store)
        };
        assert_eq!(rejected.unwrap_err().code, expected);
        assert_eq!(store.bytes(), before);
        assert_eq!(snapshot(&host, "a"), view);
        assert_eq!(host.state_usage().unwrap(), usage);
        assert!(!host.poisoned());
        if expected != "run_limit" {
            host.apply(
                Event::Cancel {
                    run: "b".into(),
                    node: None,
                },
                &mut store,
            )
            .unwrap();
        }
        let work = host.apply(capacity(1), &mut store).unwrap();
        assert_eq!(work.len(), 1);
        assert_eq!(attempt(&host, "a"), AttemptId(1));
        assert_eq!(work[0].execution, ExecutionId(2));
        assert!(store.publications().is_empty());
    }
}

#[test]
fn resource_holds_do_not_allocate_records_or_spend_the_record_budget() {
    let graph = graph();
    let mut engine = Engine::new([graph.clone()])
        .unwrap()
        .with_state_limits(StateLimits {
            runs: 2,
            attempts: 1,
            executions: 1,
        })
        .unwrap();
    engine.apply(event(&graph, "a", "a")).unwrap();
    engine.apply(event(&graph, "b", "b")).unwrap();
    let work = engine.apply(capacity(1)).unwrap();
    assert_eq!(work.len(), 1);
    assert_eq!(
        engine.state_usage().unwrap(),
        StateUsage {
            runs: 2,
            attempts: 1,
            executions: 1
        }
    );
    assert_eq!(
        engine.inspect("b").unwrap().nodes["first"],
        Disposition::Held
    );
    assert_eq!(engine.apply(capacity(1)).unwrap(), work);
    assert_eq!(engine.apply(capacity(2)).unwrap_err().code, "attempt_limit");
    assert_eq!(
        engine.inspect("b").unwrap().nodes["first"],
        Disposition::Held
    );
    let mut exhausted = Engine::new([graph.clone()]).unwrap();
    exhausted.apply(event(&graph, "a", "a")).unwrap();
    exhausted.state.next_id = u64::MAX;
    assert_eq!(
        exhausted.apply(capacity(0)).unwrap_err().code,
        "identity_limit"
    );
}

#[test]
fn shared_and_retained_results_use_one_execution_but_independent_attempts_and_authority() {
    let dir = tempfile::tempdir().unwrap();
    let mut store = SqlStore::open(&dir.path().join("owner.db"));
    let graph = graph();
    let mut host = DurableEngine::create(
        [graph.clone()],
        limits(StateLimits {
            runs: 3,
            attempts: 4,
            executions: 1,
        }),
        &mut store,
    )
    .unwrap();
    for run in ["a", "b"] {
        start(&mut host, &mut store, &graph, run, "shared").unwrap();
    }
    let work = host.apply(capacity(1), &mut store).unwrap().remove(0);
    let a = attempt(&host, "a");
    let b = attempt(&host, "b");
    let _invocation = host.claim("a", "first", a, &mut store).unwrap();
    host.apply(
        Event::Settle {
            execution: work.execution,
            outcome: Ok(values(41)),
        },
        &mut store,
    )
    .unwrap();
    host.adopt("a", "first", a, &mut store).unwrap();
    start(&mut host, &mut store, &graph, "c", "shared").unwrap();
    assert!(host.apply(capacity(0), &mut store).unwrap().is_empty());
    assert_eq!(
        host.state_usage().unwrap(),
        StateUsage {
            runs: 3,
            attempts: 3,
            executions: 1
        }
    );
    assert_eq!(
        host.inspect("c").unwrap().attempts["first"][0].acquisition,
        Acquisition::Retained
    );
    store.authorize("b", "changed");
    assert_eq!(
        host.adopt("b", "first", b, &mut store).unwrap_err().code,
        "authority_changed"
    );
    let c = attempt(&host, "c");
    host.adopt("c", "first", c, &mut store).unwrap();
    assert_eq!(store.publications().len(), 2);
    host.apply(demand("a", "second"), &mut store).unwrap();
    let before = store.bytes();
    assert_eq!(
        host.apply(capacity(1), &mut store).unwrap_err().code,
        "execution_limit"
    );
    assert_eq!(store.bytes(), before);
}

#[test]
fn zero_limits_allow_an_empty_owner_and_refuse_new_records() {
    let dir = tempfile::tempdir().unwrap();
    let mut store = SqlStore::open(&dir.path().join("owner.db"));
    let graph = graph();
    let mut host = DurableEngine::create(
        [graph.clone()],
        limits(StateLimits {
            runs: 0,
            attempts: 0,
            executions: 0,
        }),
        &mut store,
    )
    .unwrap();
    let before = store.bytes();
    assert_eq!(
        start(&mut host, &mut store, &graph, "a", "scope")
            .unwrap_err()
            .code,
        "run_limit"
    );
    assert_eq!(store.bytes(), before);
    assert!(!host.compact(&mut store).unwrap());
}
