use super::{durable_store::SqlStore, *};

fn shape() -> Shape {
    Shape::Record(BTreeMap::from([
        ("measurements".into(), Shape::Map(Box::new(Shape::Number))),
        ("label".into(), Shape::Nullable(Box::new(Shape::Text))),
    ]))
}
fn sample() -> Values {
    BTreeMap::from([(
        "value".into(),
        // Non-binary decimal timings must survive the checkpoint reader bit for
        // bit; an approximate JSON float reader rejects its own checksum.
        json!({"measurements":{"a":0.125,"b":2,"timing":916.3588435374149,
            "negative":-1759.9096371882085,"small":0.0000017599096371882085},"label":null}),
    )])
}
fn graph() -> Arc<Executable> {
    let mut registry = registry();
    registry.types.insert(contract("integer"), shape());
    registry
        .operations
        .get_mut(&contract("increment"))
        .unwrap()
        .1 = Arc::new(|_, input| Box::pin(async move { Ok(input) }));
    Arc::new(registry.compile(definition()).unwrap())
}
fn limits() -> DurableLimits {
    DurableLimits {
        record_reads: record_read_limits(),
        state: state_limits(),
        checkpoint: CheckpointLimits {
            bytes: 1_000_000,
            events: 100,
        },
        settlement_event_bytes: 16384,
        history: history_limits(),
    }
}

#[test]
fn value_constructors_are_closed_and_null_is_not_absence() {
    assert!(shape().accepts(&sample()["value"]));
    for value in [
        json!({}),
        json!({"label":null}),
        json!({"measurements":{},"label":null,"extra":0}),
        json!({"measurements":{"a":"0.1"},"label":null}),
        json!({"measurements":[],"label":null}),
        json!({"measurements":{},"label":false}),
    ] {
        assert!(!shape().accepts(&value));
    }
    assert!(Shape::Map(Box::new(Shape::Integer)).accepts(&json!({})));
    assert!(!Shape::Map(Box::new(Shape::Integer)).accepts(&json!(null)));
    assert!(!Shape::Integer.accepts(&json!(0.5)));
    for value in [json!("1"), json!(true), json!(null)] {
        assert!(!Shape::Number.accepts(&value));
    }
    assert!(Shape::Number.accepts(&json!(-1.25)));
    assert!(Shape::Number.accepts(&json!(1)));
    let graph = graph();
    let mut engine = Engine::new([graph.clone()]).unwrap();
    assert!(
        engine
            .apply(Event::Begin {
                run: "missing".into(),
                artifact: graph.identity().into(),
                inputs: BTreeMap::new(),
                scope: "scope".into(),
                policy: BTreeMap::new()
            })
            .is_err()
    );
    engine
        .apply(Event::Begin {
            run: "run".into(),
            artifact: graph.identity().into(),
            inputs: sample(),
            scope: "scope".into(),
            policy: BTreeMap::new(),
        })
        .unwrap();
    assert_eq!(
        engine.inspect("run").unwrap().artifact.types[&contract("integer")],
        shape()
    );
}

#[test]
fn overlapping_numeric_shapes_do_not_introduce_contract_coercion() {
    let mut registry = registry();
    registry
        .define_type(contract("number"), Shape::Number)
        .unwrap();
    let old = registry.compile(definition()).unwrap();
    let mut d = definition();
    d.inputs.get_mut("value").unwrap().contract = contract("number");
    assert_eq!(registry.compile(d).err().unwrap().code, "incompatible_port");
    let encoded = serde_json::to_string(&old.artifact().types[&contract("integer")]).unwrap();
    assert_eq!(encoded, "\"Integer\"");
    registry.types.insert(contract("integer"), Shape::Number);
    assert_ne!(
        old.identity(),
        registry.compile(definition()).unwrap().identity()
    );
}

#[tokio::test]
async fn format_nine_preserves_values_through_execution_compaction_and_recovery() {
    let dir = tempfile::tempdir().unwrap();
    let mut store = SqlStore::open(&dir.path().join("owner.db"));
    let graph = graph();
    let mut host = DurableEngine::create([graph.clone()], limits(), &mut store).unwrap();
    let checkpoint =
        |store: &SqlStore| Checkpoint::decode(&store.bytes(), limits().checkpoint).unwrap();
    assert_eq!(
        serde_json::from_slice::<serde_json::Value>(&store.bytes()).unwrap()["payload"]["format"],
        9
    );
    store.authorize("run", "scope");
    store
        .conn
        .execute(
            "UPDATE authority SET input=?1 WHERE run='run'",
            [serde_json::to_string(&sample()).unwrap()],
        )
        .unwrap();
    host.apply(
        Event::Begin {
            run: "run".into(),
            artifact: graph.identity().into(),
            inputs: sample(),
            scope: "scope".into(),
            policy: BTreeMap::new(),
        },
        &mut store,
    )
    .unwrap();
    for node in ["first", "second"] {
        let work = host.apply(capacity(1), &mut store).unwrap();
        let attempt = host.inspect("run").unwrap().attempts[node]
            .last()
            .unwrap()
            .id;
        if let Some(work) = work.first() {
            let report = host
                .claim("run", node, attempt, &mut store)
                .unwrap()
                .execute(evidence_limits())
                .await;
            assert_eq!(report.outcome.as_ref().unwrap(), &sample());
            host.apply(
                Event::Settle {
                    execution: work.execution,
                    outcome: report.outcome,
                },
                &mut store,
            )
            .unwrap();
        } else {
            assert_eq!(node, "second");
            assert!(matches!(
                host.inspect("run").unwrap().attempts[node]
                    .last()
                    .unwrap()
                    .state,
                AttemptState::Available
            ));
        }
        host.adopt("run", node, attempt, &mut store).unwrap();
    }
    host.compact(&mut store).unwrap();
    let cp = checkpoint(&store);
    assert_eq!(
        serde_json::from_slice::<serde_json::Value>(cp.bytes()).unwrap()["payload"]["format"],
        9
    );
    let recovered = DurableEngine::recover(cp, [graph.clone()], limits(), &mut store).unwrap();
    assert_eq!(recovered.outputs("run").unwrap().unwrap(), sample());
    assert_eq!(recovered.inspect("run").unwrap().artifact, graph.artifact());
    let cp = checkpoint(&store);
    let definition = cp
        .inspection_definition(
            graph.identity(),
            ExportLimits {
                bytes: 1_000_000,
                attempts: 100,
            },
        )
        .unwrap();
    assert!(
        serde_json::to_string(&definition)
            .unwrap()
            .contains("Nullable")
    );
    for format in 5..=8 {
        let mut cp = checkpoint(&store);
        let crate::ai::graph::checkpoint_format::Payload::Evidence(p) = &mut cp.envelope.payload
        else {
            panic!("format nine")
        };
        p.format = format;
        cp.envelope.checksum = crate::ai::graph::compile::digest(&cp.envelope.payload).unwrap();
        assert_eq!(
            Checkpoint::decode(
                &serde_json::to_vec(&cp.envelope).unwrap(),
                limits().checkpoint
            )
            .err()
            .unwrap()
            .code,
            "checkpoint_version"
        );
    }
}
