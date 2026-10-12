use super::*;
use crate::ai::{graph::*, transport::graph_identity, workspace_graph};
use serde_json::json;
use std::{collections::BTreeMap, sync::Arc};

fn graph() -> Arc<Executable> {
    let mut registry = Registry::default();
    let value = Contract::new("usage.value", 1);
    let operation = Contract::new("usage.provider", 1);
    registry.define_type(value.clone(), Shape::Integer).unwrap();
    let ports = BTreeMap::from([(
        "value".into(),
        Port {
            contract: value,
            optional: false,
        },
    )]);
    registry
        .register(
            Operation {
                contract: operation.clone(),
                implementation: "usage/provider/1".into(),
                inputs: ports.clone(),
                outputs: ports.clone(),
                resource: Resource::Provider,
                reuse: Reuse::Exact,
            },
            Arc::new(|context, inputs| {
                Box::pin(async move {
                    context.observe(ResponseEvidence {
                        request_id: Some("usage-request".into()),
                        usage: Some(UsageEvidence {
                            input_tokens: Some(7),
                            output_tokens: Some(11),
                            total_tokens: Some(18),
                            provenance: "provider".into(),
                        }),
                        ..Default::default()
                    })?;
                    Ok(inputs)
                })
            }),
        )
        .unwrap();
    Arc::new(
        workspace_graph::single_operation(
            registry,
            Contract::new("usage.graph", 1),
            operation,
            ports.clone(),
            ports,
        )
        .unwrap(),
    )
}

#[tokio::test]
async fn shared_graph_usage_counts_once_per_scope_and_survives_restart() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("usage.sqlite3");
    let mut store = Store::open(&path).unwrap();
    let workspace = store.snapshot().unwrap().learner.id;
    let graph = graph();
    let mut runtime = workspace_graph::Runtime::default();
    let mut submitted = 0;
    for (run, language) in [("one", "spanish"), ("two", "spanish"), ("three", "french")] {
        let catalog = runtime
            .begin(
                &mut store.connection,
                &workspace,
                graph.clone(),
                run,
                BTreeMap::from([("value".into(), json!(1))]),
                "shared",
                "reading",
                &json!({"scope":{"language":language}}),
                |_, _| Ok(()),
            )
            .unwrap();
        loop {
            let progress = runtime
                .poll(
                    &mut store.connection,
                    &catalog,
                    run,
                    Capacity {
                        local: 8,
                        provider: 8,
                    },
                    |db, request| {
                        if matches!(request.intent, CommitIntent::Dispatch { .. }) {
                            graph_identity::bind_workspace(db, request).unwrap();
                        }
                        Ok(())
                    },
                )
                .unwrap();
            match progress {
                workspace_graph::Progress::Invoke(invocation) => {
                    submitted += 1;
                    let report = invocation
                        .execute(EvidenceLimits {
                            observations: 8,
                            bytes: 8192,
                        })
                        .await;
                    runtime
                        .event(
                            &mut store.connection,
                            &catalog,
                            Event::SettleObserved(report),
                            |_, _| Ok(()),
                        )
                        .unwrap();
                }
                workspace_graph::Progress::Complete(_) => break,
                workspace_graph::Progress::Waiting => (),
                _ => panic!("Unexpected graph disposition"),
            }
        }
    }
    assert_eq!(submitted, 1);
    // Admission alone cannot add a provider call to the usage report.
    runtime
        .begin(
            &mut store.connection,
            &workspace,
            graph,
            "queued",
            BTreeMap::from([("value".into(), json!(2))]),
            "shared",
            "reading",
            &json!({"scope":{"language":"spanish"}}),
            |_, _| Ok(()),
        )
        .unwrap();
    for _ in 0..2 {
        let report = store.profile().unwrap();
        assert_eq!(
            (
                report.global.attempts,
                report.global.input_tokens,
                report.global.output_tokens,
                report.global.unknown_usage
            ),
            (1, 7, 11, 0)
        );
        for language in ["spanish", "french"] {
            let scope = report
                .languages
                .iter()
                .find(|row| row.id == language)
                .unwrap();
            assert_eq!(
                (scope.attempts, scope.input_tokens, scope.output_tokens),
                (1, 7, 11)
            );
        }
        assert!(report.personas.iter().all(|row| row.attempts == 0));
        crate::ai::results::set_capacity(&store.connection, 0).unwrap();
    }
    drop(store);
    let reopened = Store::open(&path).unwrap();
    assert_eq!(reopened.profile().unwrap().global.attempts, 1);
    let receipt = workspace_graph::receipt(&reopened.connection, "one")
        .unwrap()
        .unwrap();
    assert_eq!(receipt["response"]["providerId"], "usage-request");
}
