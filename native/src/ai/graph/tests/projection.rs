use super::super::projection::Disclosure;
use super::*;

fn limits() -> ExportLimits {
    ExportLimits {
        bytes: 1_000_000,
        attempts: 100,
    }
}
fn stamp(engine: &Engine) -> Stamp {
    Stamp {
        engine: uuid::Uuid::new_v4().to_string(),
        revision: engine.revision(),
        checksum: "fixture".into(),
    }
}

// Independent JSON comparison: only Constant.value may differ in the artifact.
fn conceal_constants(v: &mut serde_json::Value) {
    match v {
        serde_json::Value::Object(fields) => {
            if let Some(serde_json::Value::Object(constant)) = fields.get_mut("Constant") {
                constant.insert("value".into(), json!({"omitted":"Content"}));
            }
            for child in fields.values_mut() {
                conceal_constants(child);
            }
        }
        serde_json::Value::Array(items) => {
            for child in items {
                conceal_constants(child);
            }
        }
        _ => (),
    }
}

#[test]
fn nested_export_preserves_every_binding_and_omits_all_constant_values() {
    let mut r = registry();
    r.define_type(contract("text"), Shape::Text).unwrap();
    let mut child = definition();
    child.nodes.get_mut("second").unwrap().activation = Activation::OnDemand;
    child.nodes.insert(
        "disabled".into(),
        Node {
            activation: Activation::Disabled,
            ..node(Source::Constant {
                contract: contract("integer"),
                value: json!(483998),
            })
        },
    );
    child.outputs.insert(
        "private".into(),
        Port {
            contract: contract("text"),
            optional: false,
        },
    );
    child.results.insert(
        "private".into(),
        Source::Constant {
            contract: contract("text"),
            value: json!("private nested prompt sk-fixture-secret"),
        },
    );
    let child = r.compile(child).unwrap();
    let mut parent = definition();
    parent.nodes.clear();
    parent.results.clear();
    parent.outputs = child.artifact().definition.outputs.clone();
    parent.results = parent
        .compose(
            "child",
            &child,
            BTreeMap::from([(
                "value".into(),
                Source::Constant {
                    contract: contract("integer"),
                    value: json!(867923),
                },
            )]),
        )
        .unwrap();
    let graph = Arc::new(r.compile(parent).unwrap());
    let mut engine = Engine::new([graph.clone()]).unwrap();
    begin(&mut engine, &graph, "run", 781193);
    let before = engine.journal().to_vec();
    let snapshot = engine.project(&stamp(&engine), "run", limits()).unwrap();
    let mut expected = serde_json::to_value(graph.artifact()).unwrap();
    conceal_constants(&mut expected);
    assert_eq!(serde_json::to_value(&snapshot.artifact).unwrap(), expected);
    assert_eq!(snapshot.artifact_id, graph.identity());
    assert_eq!(snapshot.nodes.len(), 3);
    assert_eq!(snapshot.nodes["child/disabled"], Disposition::Disabled);
    assert_eq!(snapshot.nodes["child/second"], Disposition::Unrequested);
    let json = serde_json::to_string(&snapshot).unwrap();
    for secret in [
        "private nested prompt",
        "sk-fixture-secret",
        "867923",
        "483998",
        "781193",
        "test-authority-v1",
    ] {
        assert!(!json.contains(secret), "leaked {secret}");
    }
    assert_eq!(engine.journal(), before);
    assert_eq!(
        graph.artifact().definition.results["private"],
        Source::Constant {
            contract: contract("text"),
            value: json!("private nested prompt sk-fixture-secret")
        }
    );
}

#[test]
fn definition_only_and_every_run_overlay_share_identical_structure() {
    let graph = Arc::new(registry().compile(definition()).unwrap());
    let definition = graph.inspection_definition(limits()).unwrap();
    assert_eq!(definition.artifact_id, graph.identity());
    let before_runs = serde_json::to_value(&definition.artifact).unwrap();
    let serialized = serde_json::to_value(&definition).unwrap();
    for field in ["run", "revision", "attempts", "nodes", "reasons"] {
        assert!(serialized.get(field).is_none());
    }
    let mut engine = Engine::new([graph.clone()]).unwrap();
    for run in ["a", "b"] {
        begin(&mut engine, &graph, run, 40);
        engine.apply(capacity(1)).unwrap();
        let snapshot = engine.project(&stamp(&engine), run, limits()).unwrap();
        assert_eq!(
            serde_json::to_value(snapshot.artifact).unwrap(),
            before_runs
        );
    }
    assert_eq!(
        graph
            .inspection_definition(ExportLimits {
                bytes: 10,
                attempts: 0
            })
            .err()
            .unwrap()
            .code,
        "inspection_limit"
    );
}

#[test]
fn attempt_and_execution_ids_are_lossless_strings_and_state_is_native() {
    let graph = Arc::new(registry().compile(definition()).unwrap());
    let mut engine = Engine::new([graph.clone()]).unwrap();
    begin(&mut engine, &graph, "run", 40);
    engine.state.next_id = 9_007_199_254_740_991;
    let work = engine.apply(capacity(1)).unwrap().remove(0);
    engine
        .apply(Event::Pause {
            run: "run".into(),
            paused: true,
        })
        .unwrap();
    let snapshot = engine.project(&stamp(&engine), "run", limits()).unwrap();
    assert!(snapshot.paused);
    assert!(snapshot.active);
    assert_eq!(snapshot.nodes, engine.inspect("run").unwrap().nodes);
    assert_eq!(
        snapshot.activation,
        *engine.inspect("run").unwrap().activation
    );
    let a = &snapshot.attempts["first"][0];
    assert_eq!(a.id, "9007199254740992");
    assert_eq!(a.execution, work.execution.0.to_string());
    assert_eq!(a.state, AttemptState::Prepared);
    assert_eq!(a.acquisition, Acquisition::Produced);
    let json = serde_json::to_value(&snapshot).unwrap();
    assert_eq!(
        json["attempts"]["first"][0]["id"],
        json!("9007199254740992")
    );
    assert!(json["revision"].is_string());
    assert!(
        snapshot.reasons["first"]
            .iter()
            .any(|r| matches!(r,Reason::Attempt {id,..} if id==&a.id))
    );
}

#[test]
fn failure_export_keeps_known_machine_evidence_and_marks_unclassified_fields() {
    for known in [true, false] {
        let graph = Arc::new(registry().compile(definition()).unwrap());
        let mut engine = Engine::new([graph.clone()]).unwrap();
        begin(&mut engine, &graph, "run", 40);
        let work = advance(&mut engine, capacity(1)).remove(0);
        let outcome = if known {
            Ok(BTreeMap::from([(
                "value".into(),
                json!("private malformed response"),
            )]))
        } else {
            Err(unclassified(
                "private provider echo sk-fixture-secret",
                "private prompt path",
            ))
        };
        engine
            .apply(Event::Settle {
                execution: work.execution,
                outcome,
            })
            .unwrap();
        let snapshot = engine.project(&stamp(&engine), "run", limits()).unwrap();
        let AttemptState::Failed(f) = &snapshot.attempts["first"][0].state else {
            panic!("failure missing")
        };
        if known {
            assert_eq!(f.code, Disclosure::Known(CoreFaultCode::InvalidValue));
            assert_eq!(f.path, Disclosure::Known("value".into()));
        } else {
            assert_eq!(f.code, Disclosure::Omitted(Omission::Unclassified));
            assert_eq!(f.path, Disclosure::Omitted(Omission::Unclassified));
            assert!(
                matches!(&engine.inspect("run").unwrap().attempts["first"][0].state,AttemptState::Failed(f) if f.code.contains("private provider echo"))
            );
        }
        let exported = serde_json::to_string(&snapshot).unwrap();
        assert!(!exported.contains("private"));
        assert!(!exported.contains("sk-fixture-secret"));
        assert_eq!(snapshot.nodes["second"], Disposition::Blocked);
        assert!(snapshot.reasons["second"].contains(&Reason::Input {
            port: "value".into(),
            producer: Some("first".into()),
            availability: Availability::Blocked
        }));
    }
}

#[test]
fn projection_limits_fail_without_truncating_structure_or_changing_state() {
    let graph = Arc::new(registry().compile(definition()).unwrap());
    let mut engine = Engine::new([graph.clone()]).unwrap();
    begin(&mut engine, &graph, "run", 40);
    engine.apply(capacity(1)).unwrap();
    let stamp = stamp(&engine);
    let before = engine.journal().to_vec();
    assert_eq!(
        engine
            .project(
                &stamp,
                "run",
                ExportLimits {
                    bytes: 10,
                    attempts: 100
                }
            )
            .err()
            .unwrap()
            .code,
        "inspection_limit"
    );
    assert_eq!(
        engine
            .project(
                &stamp,
                "run",
                ExportLimits {
                    bytes: 1_000_000,
                    attempts: 0
                }
            )
            .err()
            .unwrap()
            .code,
        "inspection_limit"
    );
    let mut stale = stamp.clone();
    stale.revision -= 1;
    assert_eq!(
        engine.project(&stale, "run", limits()).err().unwrap().code,
        "inspection_revision"
    );
    assert_eq!(engine.journal(), before);
    assert_eq!(
        engine.project(&stamp, "run", limits()).unwrap().nodes.len(),
        2
    );
}

#[test]
fn a_sensitive_path_does_not_erase_a_known_failure_code() {
    let graph = Arc::new(registry().compile(definition()).unwrap());
    let mut engine = Engine::new([graph.clone()]).unwrap();
    begin(&mut engine, &graph, "run", 40);
    let work = advance(&mut engine, capacity(1)).remove(0);
    engine
        .apply(Event::Settle {
            execution: work.execution,
            outcome: Err(unclassified("invalid_value", "private echoed response")),
        })
        .unwrap();
    let snapshot = engine.project(&stamp(&engine), "run", limits()).unwrap();
    let AttemptState::Failed(f) = &snapshot.attempts["first"][0].state else {
        panic!("failure missing")
    };
    assert_eq!(f.code, Disclosure::Known(CoreFaultCode::InvalidValue));
    assert_eq!(f.path, Disclosure::Omitted(Omission::Unclassified));
    assert!(
        !serde_json::to_string(&snapshot)
            .unwrap()
            .contains("private echoed response")
    );
}

#[test]
fn durable_snapshot_is_revision_bound_and_refuses_uncertain_hosts() {
    let dir = tempfile::tempdir().unwrap();
    let mut store = durable_store::SqlStore::open(&dir.path().join("owner.db"));
    let graph = Arc::new(registry().compile(definition()).unwrap());
    let mut host = DurableEngine::create(
        [graph.clone()],
        DurableLimits {
            record_reads: record_read_limits(),
            checkpoint: CheckpointLimits {
                bytes: 1_000_000,
                events: 100,
            },
            settlement_event_bytes: 4096,
            history: history_limits(),
            state: state_limits(),
        },
        &mut store,
    )
    .unwrap();
    store.authorize("run", "scope");
    host.apply(
        Event::Begin {
            run: "run".into(),
            artifact: graph.identity().into(),
            scope: "scope".into(),
            inputs: values(40),
            policy: BTreeMap::new(),
        },
        &mut store,
    )
    .unwrap();
    let snapshot = host.inspection_snapshot("run", limits()).unwrap();
    assert_eq!(snapshot.engine, host.stamp().engine);
    assert_eq!(snapshot.revision, host.stamp().revision.to_string());
    let control = host.run_control("run").unwrap();
    assert_eq!(control.active, snapshot.active);
    assert_eq!(control.paused, snapshot.paused);
    assert!(host.run_control("missing").is_err());
    for paused in [true, false] {
        host.apply(
            Event::Pause {
                run: "run".into(),
                paused,
            },
            &mut store,
        )
        .unwrap();
        assert_eq!(host.run_control("run").unwrap().paused, paused);
    }
    store.fail_after_commit = true;
    assert!(host.apply(capacity(1), &mut store).is_err());
    assert_eq!(
        host.run_control("run").err().unwrap().code,
        "reload_required"
    );
    assert_eq!(
        host.inspection_snapshot("run", limits())
            .err()
            .unwrap()
            .code,
        "reload_required"
    );
}
