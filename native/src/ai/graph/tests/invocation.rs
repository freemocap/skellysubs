use super::*;
use std::sync::Mutex;

fn response() -> ResponseEvidence {
    ResponseEvidence {
        request_id: Some("request-123".into()),
        requested_model: Some("requested".into()),
        actual_model: Some("actual".into()),
        finish_reason: Some("stop".into()),
        usage: Some(UsageEvidence {
            input_tokens: Some(12),
            output_tokens: None,
            total_tokens: None,
            provenance: "response-usage".into(),
        }),
        billing: Some(BillingEvidence {
            amount: "0.0100".into(),
            currency: "USD".into(),
            provenance: "admission".into(),
            basis: BillingBasis::Allowance,
        }),
        additional: BTreeMap::from([
            (
                "structured".into(),
                EvidenceValue::ClassifiedJson(
                    json!({"usage": {"cost": 0.01}, "timings": [1, null]}),
                ),
            ),
            (
                "extension".into(),
                EvidenceValue::Omitted(EvidenceOmission::Unclassified),
            ),
            (
                "prompt".into(),
                EvidenceValue::Omitted(EvidenceOmission::Content),
            ),
        ]),
        ..ResponseEvidence::default()
    }
}

fn graph_with(handler: Handler) -> Arc<Executable> {
    let r = registry();
    let operation = r.operations[&contract("increment")].0.clone();
    let mut r = Registry::default();
    r.define_type(contract("integer"), Shape::Integer).unwrap();
    r.register(operation, handler).unwrap();
    Arc::new(r.compile(definition()).unwrap())
}

fn claim(graph: &Arc<Executable>) -> (Engine, Work, Invocation) {
    let mut engine = Engine::new([graph.clone()]).unwrap();
    begin(&mut engine, graph, "a", 40);
    let work = engine.apply(capacity(1)).unwrap().remove(0);
    let invocation = engine.claim(work.execution).unwrap();
    (engine, work, invocation)
}

#[tokio::test]
async fn provider_evidence_survives_handler_failure_and_output_validation_failure() {
    for fail_handler in [false, true] {
        let graph = graph_with(Arc::new(move |context, _| {
            Box::pin(async move {
                context.observe(response())?;
                if fail_handler {
                    Err(unclassified("transport_failure", "response"))
                } else {
                    Ok(BTreeMap::from([("value".into(), json!("invalid integer"))]))
                }
            })
        }));
        let (_, work, invocation) = claim(&graph);
        let report = invocation.execute(evidence_limits()).await;
        assert_eq!(report.identity.execution, work.execution);
        assert_eq!(report.identity.artifact, graph.identity());
        assert_eq!(report.identity.operation, contract("increment"));
        assert_eq!(
            report.outcome.unwrap_err().code,
            if fail_handler {
                "transport_failure"
            } else {
                "invalid_value"
            }
        );
        assert_eq!(report.observations, vec![response()]);
        assert!(report.evidence_failure.is_none());
        let evidence = &report.observations[0];
        assert_eq!(evidence.usage.as_ref().unwrap().output_tokens, None);
        assert_eq!(
            evidence.billing.as_ref().unwrap().basis,
            BillingBasis::Allowance
        );
    }
}

#[tokio::test]
async fn partial_evidence_keeps_arrival_order_and_explicit_omissions_on_success() {
    let graph = graph_with(Arc::new(|context, input| {
        Box::pin(async move {
            context.observe(ResponseEvidence {
                requested_model: Some("requested".into()),
                ..ResponseEvidence::default()
            })?;
            context.observe(response())?;
            Ok(input)
        })
    }));
    let (_, _, invocation) = claim(&graph);
    let report = invocation.execute(evidence_limits()).await;
    assert_eq!(report.outcome.unwrap(), values(40));
    assert_eq!(report.observations.len(), 2);
    assert_eq!(report.observations[0].request_id, None);
    assert_eq!(report.observations[1], response());
    // Evidence is not injected into required output ports or the artifact.
    assert_eq!(
        graph.artifact().operations[&contract("increment")].outputs,
        ports()
    );
}

#[tokio::test]
async fn budgets_preserve_accepted_evidence_and_ignored_capture_errors_fail_success() {
    let bytes = serde_json::to_vec(&response()).unwrap().len();
    for (limits, accepted) in [
        (
            EvidenceLimits {
                observations: 2,
                bytes: bytes * 2,
            },
            2,
        ),
        (
            EvidenceLimits {
                observations: 2,
                bytes: bytes * 2 - 1,
            },
            1,
        ),
        (
            EvidenceLimits {
                observations: 1,
                bytes: bytes * 2,
            },
            1,
        ),
        (
            EvidenceLimits {
                observations: 0,
                bytes: bytes * 2,
            },
            0,
        ),
        (
            EvidenceLimits {
                observations: 2,
                bytes: 0,
            },
            0,
        ),
    ] {
        let graph = graph_with(Arc::new(|context, input| {
            Box::pin(async move {
                // Deliberately bad adapter ignores both failures; it still cannot
                // return a successful report with silently missing evidence.
                let _ = context.observe(response());
                let _ = context.observe(response());
                Ok(input)
            })
        }));
        let (_, _, invocation) = claim(&graph);
        let report = invocation.execute(limits).await;
        assert_eq!(report.observations, vec![response(); accepted]);
        if accepted == 2 {
            assert_eq!(report.outcome.unwrap(), values(40));
            assert!(report.evidence_failure.is_none());
        } else {
            assert_eq!(report.outcome.unwrap_err().code, "evidence_limit");
            assert_eq!(report.evidence_failure.unwrap().code, "evidence_limit");
        }
    }
}

#[tokio::test]
async fn evidence_limit_does_not_replace_original_handler_failure() {
    let graph = graph_with(Arc::new(|context, _| {
        Box::pin(async move {
            context.observe(response()).unwrap();
            assert!(context.observe(response()).is_err());
            Err(unclassified("remote_error", "response"))
        })
    }));
    let (_, _, invocation) = claim(&graph);
    let report = invocation
        .execute(EvidenceLimits {
            observations: 1,
            bytes: 32_768,
        })
        .await;
    assert_eq!(report.outcome.unwrap_err().code, "remote_error");
    assert_eq!(report.evidence_failure.unwrap().code, "evidence_limit");
    assert_eq!(report.observations, vec![response()]);
}

#[tokio::test]
async fn shared_execution_has_one_identity_and_late_callbacks_cannot_rewrite_report() {
    let saved = Arc::new(Mutex::new(None));
    let callback = saved.clone();
    let graph = graph_with(Arc::new(move |context, input| {
        *callback.lock().unwrap() = Some(context.clone());
        Box::pin(async move {
            context.observe(response())?;
            Ok(input)
        })
    }));
    let (mut engine, work, invocation) = claim(&graph);
    begin(&mut engine, &graph, "b", 40);
    assert!(engine.apply(capacity(1)).unwrap().is_empty());
    engine
        .apply(Event::Cancel {
            run: "a".into(),
            node: None,
        })
        .unwrap();
    let report = invocation.execute(evidence_limits()).await;
    let context = saved.lock().unwrap().take().unwrap();
    assert_eq!(context.identity(), &report.identity);
    assert_eq!(context.snapshot().unwrap_err().code, "invocation_closed");
    assert_eq!(
        context.observe(response()).unwrap_err().code,
        "invocation_closed"
    );
    assert_eq!(report.observations, vec![response()]);
    engine
        .apply(Event::Settle {
            execution: work.execution,
            outcome: report.outcome,
        })
        .unwrap();
    assert_eq!(
        engine.disposition("a", "first").unwrap(),
        Disposition::Cancelled
    );
    assert_eq!(
        engine.disposition("b", "first").unwrap(),
        Disposition::Available
    );
}

#[tokio::test]
async fn dropping_running_future_closes_retained_callback() {
    let (send, receive) = tokio::sync::oneshot::channel();
    let send = Mutex::new(Some(send));
    let graph = graph_with(Arc::new(move |context, _| {
        send.lock()
            .unwrap()
            .take()
            .unwrap()
            .send(context.clone())
            .ok()
            .unwrap();
        Box::pin(async move {
            context.observe(response())?;
            std::future::pending::<Result<Values>>().await
        })
    }));
    let (_, _, invocation) = claim(&graph);
    let task = tokio::spawn(invocation.execute(evidence_limits()));
    let context = receive.await.unwrap();
    task.abort();
    assert!(task.await.err().unwrap().is_cancelled());
    assert_eq!(
        context.observe(response()).unwrap_err().code,
        "invocation_closed"
    );
    assert_eq!(
        context.provisional_text("late").unwrap_err().code,
        "invocation_closed"
    );
}

#[tokio::test]
async fn provisional_capture_limits_preserve_source_metadata_and_original_failures() {
    for (bytes, fail_handler) in [(0, false), (2, false), (3, false), (2, true)] {
        let graph = graph_with(Arc::new(move |context, input| {
            Box::pin(async move {
                context.observe(response())?;
                let _ = context.provisional_text("é");
                let _ = context.provisional_text("海");
                if fail_handler {
                    Err(unclassified("transport", "response"))
                } else {
                    Ok(input)
                }
            })
        }));
        let (_, _, invocation) = claim(&graph);
        let report = invocation
            .execute_with_provisional(evidence_limits(), ProvisionalLimits { bytes })
            .await;
        assert_eq!(report.observations, vec![response()]);
        assert!(report.evidence_failure.is_none());
        let capture = report.provisional.unwrap();
        if bytes == 3 {
            assert!(report.outcome.is_ok());
            assert_eq!(capture.text, "海");
            assert_eq!(capture.sequence, 2);
            assert!(capture.failure.is_none());
        } else {
            assert_eq!(capture.text, if bytes == 0 { "" } else { "é" });
            assert_eq!(capture.sequence, if bytes == 0 { 0 } else { 1 });
            assert_eq!(capture.failure.unwrap().code, "provisional_limit");
            assert_eq!(
                report.outcome.unwrap_err().code,
                if fail_handler {
                    "transport"
                } else {
                    "provisional_limit"
                }
            );
        }
    }
    let graph = graph_with(Arc::new(|context, _| {
        Box::pin(async move {
            context.observe(response())?;
            context.provisional_text("unvalidated source")?;
            Ok(BTreeMap::from([("value".into(), json!("not an integer"))]))
        })
    }));
    let (_, _, invocation) = claim(&graph);
    let report = invocation
        .execute_with_provisional(evidence_limits(), ProvisionalLimits { bytes: 100 })
        .await;
    assert!(report.outcome.is_err());
    assert_eq!(report.observations, vec![response()]);
    assert_eq!(report.provisional.unwrap().text, "unvalidated source");
}
