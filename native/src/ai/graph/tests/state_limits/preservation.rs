use super::*;

#[test]
fn exact_record_ceiling_still_allows_dispatch_settlement_adoption_compaction_and_recovery() {
    for fail in [false, true] {
        let dir = tempfile::tempdir().unwrap();
        let mut store = SqlStore::open(&dir.path().join("owner.db"));
        let graph = graph();
        let limits = limits(StateLimits {
            runs: 1,
            attempts: 1,
            executions: 1,
        });
        let mut host = DurableEngine::create([graph.clone()], limits, &mut store).unwrap();
        start(&mut host, &mut store, &graph, "a", "scope").unwrap();
        let work = host.apply(capacity(1), &mut store).unwrap().remove(0);
        let a = attempt(&host, "a");
        let _invocation = host.claim("a", "first", a, &mut store).unwrap();
        let usage = host.state_usage().unwrap();
        host.compact(&mut store).unwrap();
        host.apply(
            Event::Settle {
                execution: work.execution,
                outcome: if fail {
                    Err(unclassified("fixture", "failure"))
                } else {
                    Ok(values(41))
                },
            },
            &mut store,
        )
        .unwrap();
        if fail {
            host.apply(
                Event::Retry {
                    run: "a".into(),
                    node: "first".into(),
                },
                &mut store,
            )
            .unwrap();
        } else {
            host.adopt("a", "first", a, &mut store).unwrap();
            host.apply(demand("a", "second"), &mut store).unwrap();
        }
        let before = store.bytes();
        assert_eq!(
            host.apply(capacity(1), &mut store).unwrap_err().code,
            "attempt_limit"
        );
        assert_eq!(store.bytes(), before);
        assert_eq!(host.state_usage().unwrap(), usage);
        host.compact(&mut store).unwrap();
        let mut host =
            DurableEngine::recover(checkpoint(&store), [graph], limits, &mut store).unwrap();
        assert_eq!(host.state_usage().unwrap(), usage);
        host.apply(
            Event::Cancel {
                run: "a".into(),
                node: None,
            },
            &mut store,
        )
        .unwrap();
        assert_eq!(host.state_usage().unwrap(), usage); // cancellation is not eviction
        assert_eq!(store.publications().len(), usize::from(!fail));
    }
}

#[test]
fn lower_replay_limits_reject_all_history_before_any_recovery_write() {
    for compact in [false, true] {
        let dir = tempfile::tempdir().unwrap();
        let mut store = SqlStore::open(&dir.path().join("owner.db"));
        let graph = graph();
        let generous = limits(state_limits());
        let mut host = DurableEngine::create([graph.clone()], generous, &mut store).unwrap();
        start(&mut host, &mut store, &graph, "a", "scope").unwrap();
        for index in 0..2 {
            let work = host.apply(capacity(1), &mut store).unwrap().remove(0);
            let a = attempt(&host, "a");
            let _invocation = host.claim("a", "first", a, &mut store).unwrap();
            host.apply(
                Event::Settle {
                    execution: work.execution,
                    outcome: Err(unclassified("fixture", "failure")),
                },
                &mut store,
            )
            .unwrap();
            if index == 0 {
                host.apply(
                    Event::Retry {
                        run: "a".into(),
                        node: "first".into(),
                    },
                    &mut store,
                )
                .unwrap();
            }
        }
        if compact {
            host.compact(&mut store).unwrap();
        }
        let before = store.bytes();
        for (state, error) in [
            (
                StateLimits {
                    runs: 0,
                    ..state_limits()
                },
                "run_limit",
            ),
            (
                StateLimits {
                    attempts: 1,
                    ..state_limits()
                },
                "attempt_limit",
            ),
            (
                StateLimits {
                    executions: 1,
                    ..state_limits()
                },
                "execution_limit",
            ),
        ] {
            assert_eq!(
                DurableEngine::recover(
                    checkpoint(&store),
                    [graph.clone()],
                    limits(state),
                    &mut store
                )
                .err()
                .unwrap()
                .code,
                error
            );
            assert_eq!(
                checkpoint(&store)
                    .historical_inspection(
                        1,
                        HistoricalLimits {
                            history: history_limits(),
                            state
                        },
                        &mut store
                    )
                    .err()
                    .unwrap()
                    .code,
                error
            );
            assert_eq!(store.bytes(), before);
        }
        let recovered =
            DurableEngine::recover(checkpoint(&store), [graph], generous, &mut store).unwrap();
        assert_eq!(
            recovered.state_usage().unwrap(),
            StateUsage {
                runs: 1,
                attempts: 2,
                executions: 2
            }
        );
        assert_eq!(recovered.inspect("a").unwrap().attempts["first"].len(), 2);
        assert!(store.publications().is_empty());
    }
}

#[test]
fn rollback_and_uncertain_commit_do_not_invent_or_release_record_capacity() {
    let dir = tempfile::tempdir().unwrap();
    let mut store = SqlStore::open(&dir.path().join("owner.db"));
    let graph = graph();
    let limits = limits(StateLimits {
        runs: 1,
        attempts: 1,
        executions: 1,
    });
    let mut host = DurableEngine::create([graph.clone()], limits, &mut store).unwrap();
    start(&mut host, &mut store, &graph, "a", "scope").unwrap();
    let before = store.bytes();
    store.fail_before_commit = true;
    assert_eq!(
        host.apply(capacity(1), &mut store).unwrap_err().code,
        "injected_rollback"
    );
    assert_eq!(host.state_usage().unwrap().attempts, 0);
    assert_eq!(store.bytes(), before);
    store.fail_before_commit = false;
    store.fail_after_commit = true;
    assert_eq!(
        host.apply(capacity(1), &mut store).unwrap_err().code,
        "acknowledgment_lost"
    );
    assert_eq!(host.state_usage().unwrap_err().code, "reload_required");
    store.fail_after_commit = false;
    let host = DurableEngine::recover(checkpoint(&store), [graph], limits, &mut store).unwrap();
    assert_eq!(
        host.state_usage().unwrap(),
        StateUsage {
            runs: 1,
            attempts: 1,
            executions: 1
        }
    );
    assert_eq!(
        host.inspect("a").unwrap().nodes["first"],
        Disposition::Failed
    );
    assert!(store.publications().is_empty());
}

#[test]
fn crash_at_the_record_ceiling_marks_running_work_unknown_without_reissuing_it() {
    let dir = tempfile::tempdir().unwrap();
    let mut store = SqlStore::open(&dir.path().join("owner.db"));
    let graph = graph();
    let limits = limits(StateLimits {
        runs: 1,
        attempts: 1,
        executions: 1,
    });
    let mut host = DurableEngine::create([graph.clone()], limits, &mut store).unwrap();
    start(&mut host, &mut store, &graph, "a", "scope").unwrap();
    host.apply(capacity(1), &mut store).unwrap();
    let a = attempt(&host, "a");
    let _invocation = host.claim("a", "first", a, &mut store).unwrap();
    host.compact(&mut store).unwrap();
    let usage = host.state_usage().unwrap();
    let mut host =
        DurableEngine::recover(checkpoint(&store), [graph.clone()], limits, &mut store).unwrap();
    assert_eq!(host.state_usage().unwrap(), usage);
    assert_eq!(
        host.inspect("a").unwrap().nodes["first"],
        Disposition::Unknown
    );
    assert!(host.claim("a", "first", a, &mut store).is_err());
    let before = store.bytes();
    DurableEngine::recover(checkpoint(&store), [graph], limits, &mut store).unwrap();
    assert_eq!(store.bytes(), before);
}
