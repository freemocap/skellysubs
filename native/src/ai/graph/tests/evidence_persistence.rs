use super::{durable_store::SqlStore, *};

mod live_capture;
mod live_read;
mod provisional;
mod structured;

#[tokio::test]
async fn rejected_adoption_does_not_discard_the_completed_execution_evidence() {
    let mut f = Fixture::new().await;
    f.host.settle_report(&f.report, &mut f.store).unwrap();
    let saved = f.evidence();
    f.store.authorize("a", "changed");
    assert_eq!(
        f.host
            .adopt("a", "first", f.attempt, &mut f.store)
            .unwrap_err()
            .code,
        "authority_changed"
    );
    assert_eq!(f.evidence(), saved);
    f.store.authorize("a", "scope");
    f.store.fail_before_commit = true;
    assert!(f.host.adopt("a", "first", f.attempt, &mut f.store).is_err());
    assert_eq!(f.evidence(), saved);
    assert!(f.store.publications().is_empty());
    f.store.fail_before_commit = false;
    f.host.adopt("a", "first", f.attempt, &mut f.store).unwrap();
    assert_eq!(f.evidence(), saved);
    assert_eq!(f.store.publications().len(), 1);
}
mod versions;

fn limits() -> DurableLimits {
    DurableLimits {
        record_reads: record_read_limits(),
        state: state_limits(),
        checkpoint: CheckpointLimits {
            bytes: 1_000_000,
            events: 100,
        },
        settlement_event_bytes: 4096,
        history: history_limits(),
    }
}

fn graph() -> Arc<Executable> {
    let mut registry = registry();
    registry
        .operations
        .get_mut(&contract("increment"))
        .unwrap()
        .1 = Arc::new(|context, values| {
        Box::pin(async move {
            context.observe(ResponseEvidence {
                requested_model: Some("requested".into()),
                ..ResponseEvidence::default()
            })?;
            context.observe(ResponseEvidence {
                request_id: Some("request-123".into()),
                actual_model: Some("actual".into()),
                finish_reason: Some("stop".into()),
                usage: Some(UsageEvidence {
                    input_tokens: Some(12),
                    output_tokens: None,
                    total_tokens: None,
                    provenance: "response".into(),
                }),
                additional: BTreeMap::from([(
                    "extension".into(),
                    EvidenceValue::Omitted(EvidenceOmission::Unclassified),
                )]),
                ..ResponseEvidence::default()
            })?;
            Ok(values)
        })
    });
    Arc::new(registry.compile(definition()).unwrap())
}

fn begin_host(host: &mut DurableEngine, store: &mut SqlStore, graph: &Executable, run: &str) {
    store.authorize(run, "scope");
    host.apply(
        Event::Begin {
            run: run.into(),
            artifact: graph.identity().into(),
            inputs: values(40),
            scope: "scope".into(),
            policy: BTreeMap::new(),
        },
        store,
    )
    .unwrap();
}

struct Fixture {
    dir: tempfile::TempDir,
    store: SqlStore,
    host: DurableEngine,
    graph: Arc<Executable>,
    attempt: AttemptId,
    report: InvocationReport,
}
impl Fixture {
    async fn new() -> Self {
        let dir = tempfile::tempdir().unwrap();
        let mut store = SqlStore::open(&dir.path().join("owner.db"));
        let graph = graph();
        let mut host = DurableEngine::create([graph.clone()], limits(), &mut store).unwrap();
        begin_host(&mut host, &mut store, &graph, "a");
        host.apply(capacity(1), &mut store).unwrap();
        let attempt = host.inspect("a").unwrap().attempts["first"][0].id;
        let report = host
            .claim("a", "first", attempt, &mut store)
            .unwrap()
            .execute(evidence_limits())
            .await;
        Self {
            dir,
            store,
            host,
            graph,
            attempt,
            report,
        }
    }
    fn prefix(&self) -> EvidenceSnapshot {
        EvidenceSnapshot {
            identity: self.report.identity.clone(),
            observations: self.report.observations[..1].to_vec(),
            evidence_failure: None,
            provisional: None,
        }
    }
    fn evidence(&mut self) -> Option<ExecutionEvidence> {
        self.host
            .read_execution_evidence(self.report.identity.execution, &mut self.store)
            .unwrap()
    }
    fn checkpoint(&self) -> Checkpoint {
        Checkpoint::decode(&self.store.bytes(), limits().checkpoint).unwrap()
    }
    fn reopen(&mut self) {
        self.store = SqlStore::open(&self.dir.path().join("owner.db"));
        self.host = DurableEngine::recover(
            self.checkpoint(),
            [self.graph.clone()],
            limits(),
            &mut self.store,
        )
        .unwrap();
    }
}

#[tokio::test]
async fn complete_reports_survive_validation_failure_eviction_archive_and_reopen() {
    for outcome in [
        Ok(values(40)),
        Ok(BTreeMap::new()),
        Err(unclassified("transport", "response")),
    ] {
        let mut f = Fixture::new().await;
        f.report.outcome = outcome.clone();
        f.host.settle_report(&f.report, &mut f.store).unwrap();
        let expected = f.evidence().unwrap();
        assert!(expected.complete);
        assert_eq!(expected.observations, f.report.observations);
        let revision = f.host.stamp().revision;
        let succeeded = outcome == Ok(values(40));
        assert_eq!(
            f.host.inspect("a").unwrap().nodes["first"],
            if succeeded {
                Disposition::Available
            } else {
                Disposition::Failed
            }
        );
        f.host.evict_records(&mut f.store).unwrap();
        assert_eq!(f.evidence(), Some(expected.clone()));
        let history = f
            .checkpoint()
            .historical_inspection(
                revision,
                HistoricalLimits {
                    history: history_limits(),
                    state: state_limits(),
                },
                &mut f.store,
            )
            .unwrap();
        assert_eq!(
            history
                .execution_evidence(f.report.identity.execution)
                .unwrap(),
            Some(expected.clone())
        );
        f.reopen();
        assert_eq!(f.evidence(), Some(expected));
        assert!(f.host.apply(capacity(1), &mut f.store).unwrap().is_empty());
        if succeeded {
            f.host
                .apply(
                    Event::Pause {
                        run: "a".into(),
                        paused: false,
                    },
                    &mut f.store,
                )
                .unwrap();
            f.host.adopt("a", "first", f.attempt, &mut f.store).unwrap();
            assert_eq!(f.store.publications().len(), 1);
        } else {
            assert!(f.host.adopt("a", "first", f.attempt, &mut f.store).is_err());
        }
    }
}

#[tokio::test]
async fn partial_observations_survive_recovery_without_fabricating_completion() {
    let mut f = Fixture::new().await;
    assert!(f.evidence().is_none());
    f.host
        .record_observations(&f.prefix(), &mut f.store)
        .unwrap();
    f.host.evict_records(&mut f.store).unwrap();
    f.reopen();
    assert_eq!(
        f.host.inspect("a").unwrap().nodes["first"],
        Disposition::Unknown
    );
    assert_eq!(f.evidence().unwrap().observations, f.prefix().observations);
    assert!(!f.evidence().unwrap().complete);
    assert!(f.host.settle_report(&f.report, &mut f.store).is_err());
    // Late metadata can be retained without making an unknown result adoptable.
    f.host
        .record_observations(
            &EvidenceSnapshot {
                identity: f.report.identity.clone(),
                observations: f.report.observations.clone(),
                evidence_failure: None,
                provisional: None,
            },
            &mut f.store,
        )
        .unwrap();
    assert!(!f.evidence().unwrap().complete);
    assert_eq!(
        f.host.inspect("a").unwrap().nodes["first"],
        Disposition::Unknown
    );
    assert!(f.store.publications().is_empty());
    assert!(f.host.apply(capacity(1), &mut f.store).unwrap().is_empty());
}

#[tokio::test]
async fn report_and_primary_record_are_one_transaction_and_ack_loss_requires_reload() {
    for failure in ["rollback", "partial_write", "ack"] {
        let mut f = Fixture::new().await;
        f.host
            .record_observations(&f.prefix(), &mut f.store)
            .unwrap();
        let before = f.store.bytes();
        let prefix = f.evidence();
        f.store.fail_before_commit = failure == "rollback";
        f.store.fail_record_after = (failure == "partial_write").then_some(1);
        f.store.fail_after_commit = failure == "ack";
        assert!(f.host.settle_report(&f.report, &mut f.store).is_err());
        if failure == "ack" {
            assert!(
                f.host
                    .read_execution_evidence(f.report.identity.execution, &mut f.store)
                    .is_err()
            );
            f.reopen();
            assert!(f.evidence().unwrap().complete);
            assert!(f.host.settle_report(&f.report, &mut f.store).is_err());
        } else {
            assert_eq!(f.store.bytes(), before);
            assert_eq!(f.evidence(), prefix);
            assert_eq!(
                f.host.inspect("a").unwrap().nodes["first"],
                Disposition::Running
            );
            f.store.fail_before_commit = false;
            f.store.fail_record_after = None;
            f.host.settle_report(&f.report, &mut f.store).unwrap();
            assert!(f.evidence().unwrap().complete);
        }
    }
}

#[tokio::test]
async fn identity_prefix_and_budget_rejections_preserve_the_committed_prefix() {
    let mut f = Fixture::new().await;
    f.host
        .record_observations(&f.prefix(), &mut f.store)
        .unwrap();
    let before = f.store.bytes();
    for mutation in [
        "engine",
        "artifact",
        "operation",
        "prefix",
        "shrink",
        "size",
    ] {
        let mut report = f.report.clone();
        match mutation {
            "engine" => report.identity.engine = Some(uuid::Uuid::new_v4().to_string()),
            "artifact" => report.identity.artifact = "wrong".into(),
            "operation" => report.identity.operation = contract("wrong"),
            "prefix" => report.observations[0].requested_model = Some("rewritten".into()),
            "shrink" => report.observations.clear(),
            _ => report.observations[1].request_id = Some("x".repeat(4096)),
        }
        assert!(
            f.host.settle_report(&report, &mut f.store).is_err(),
            "{mutation}"
        );
        assert_eq!(f.store.bytes(), before, "{mutation}");
        assert_eq!(f.evidence().unwrap().observations, f.prefix().observations);
    }
    assert_eq!(
        f.host
            .apply(
                Event::Settle {
                    execution: f.report.identity.execution,
                    outcome: Ok(values(40))
                },
                &mut f.store
            )
            .unwrap_err()
            .code,
        "evidence_required"
    );
    f.host.settle_report(&f.report, &mut f.store).unwrap();
}

#[tokio::test]
async fn cancelled_consumers_do_not_drop_shared_report_or_regain_authority() {
    let mut f = Fixture::new().await;
    begin_host(&mut f.host, &mut f.store, &f.graph, "b");
    assert!(f.host.apply(capacity(1), &mut f.store).unwrap().is_empty());
    f.host
        .apply(
            Event::Cancel {
                run: "a".into(),
                node: None,
            },
            &mut f.store,
        )
        .unwrap();
    f.host.settle_report(&f.report, &mut f.store).unwrap();
    assert!(f.evidence().unwrap().complete);
    assert_eq!(
        f.host.inspect("a").unwrap().nodes["first"],
        Disposition::Cancelled
    );
    assert_eq!(
        f.host.inspect("b").unwrap().nodes["first"],
        Disposition::Available
    );
    assert!(f.host.adopt("a", "first", f.attempt, &mut f.store).is_err());
    let b = f.host.inspect("b").unwrap().attempts["first"][0].id;
    f.host.adopt("b", "first", b, &mut f.store).unwrap();
    assert_eq!(f.store.publications().len(), 1);
}
