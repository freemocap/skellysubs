use super::*;

fn limits() -> CheckpointLimits {
    CheckpointLimits {
        bytes: 1_000_000,
        events: 100,
    }
}
fn export_limits() -> ExportLimits {
    ExportLimits {
        bytes: 1_000_000,
        attempts: 0,
    }
}

#[test]
fn recorded_definition_is_available_after_all_handlers_are_dropped() {
    let registry = registry();
    let graph = Arc::new(registry.compile(definition()).unwrap());
    let handlers: Vec<_> = graph.handlers.values().map(Arc::downgrade).collect();
    let identity = graph.identity().to_string();
    let expected =
        serde_json::to_value(graph.inspection_definition(export_limits()).unwrap()).unwrap();
    let mut engine = Engine::new([graph.clone()]).unwrap();
    begin(&mut engine, &graph, "recorded-run", 40);
    let work = advance(&mut engine, capacity(1)).remove(0);
    engine
        .apply(Event::Settle {
            execution: work.execution,
            outcome: Ok(values(41)),
        })
        .unwrap();
    adopt(&mut engine, "recorded-run", "first");
    let checkpoint =
        Checkpoint::capture(&engine, &uuid::Uuid::new_v4().to_string(), limits()).unwrap();
    let bytes = checkpoint.bytes().to_vec();
    drop(checkpoint);
    drop(engine);
    drop(graph);
    drop(registry);
    assert!(handlers.iter().all(|handler| handler.upgrade().is_none()));

    let checkpoint = Checkpoint::decode(&bytes, limits()).unwrap();
    assert_eq!(
        checkpoint.artifact_ids().collect::<Vec<_>>(),
        vec![identity.as_str()]
    );
    let stamp = checkpoint.stamp().clone();
    let actual = serde_json::to_value(
        checkpoint
            .inspection_definition(&identity, export_limits())
            .unwrap(),
    )
    .unwrap();
    assert_eq!(actual, expected);
    for field in ["run", "revision", "attempts", "nodes", "reasons"] {
        assert!(actual.get(field).is_none());
    }
    assert_eq!(checkpoint.stamp(), &stamp);
    assert_eq!(checkpoint.bytes(), bytes);
    // Evidence readability does not weaken the requirement for exact executable
    // bindings when recovering a live engine.
    assert_eq!(
        checkpoint.replay([], state_limits()).err().unwrap().code,
        "checkpoint_artifact_mismatch"
    );
}

#[test]
fn changed_current_code_cannot_rewrite_the_retained_definition() {
    let mut registry = registry();
    let original = Arc::new(registry.compile(definition()).unwrap());
    let old_id = original.identity().to_string();
    let old =
        serde_json::to_value(original.inspection_definition(export_limits()).unwrap()).unwrap();
    let engine = Engine::new([original]).unwrap();
    let checkpoint =
        Checkpoint::capture(&engine, &uuid::Uuid::new_v4().to_string(), limits()).unwrap();
    registry
        .operations
        .get_mut(&contract("increment"))
        .unwrap()
        .0
        .implementation = "synthetic/increment/v2".into();
    let mut definition = definition();
    definition
        .nodes
        .get_mut("second")
        .unwrap()
        .inputs
        .insert("value".into(), Source::Input("value".into()));
    let current = Arc::new(registry.compile(definition).unwrap());
    assert_ne!(current.identity(), old_id);
    assert_eq!(
        serde_json::to_value(
            checkpoint
                .inspection_definition(&old_id, export_limits())
                .unwrap()
        )
        .unwrap(),
        old
    );
    assert_eq!(
        checkpoint
            .inspection_definition(current.identity(), export_limits())
            .err()
            .unwrap()
            .code,
        "unknown_artifact"
    );
    assert_eq!(
        checkpoint
            .replay([current], state_limits())
            .err()
            .unwrap()
            .code,
        "checkpoint_artifact_mismatch"
    );
}

#[test]
fn nested_retained_structure_uses_the_same_sensitive_value_projection() {
    let mut registry = registry();
    registry.define_type(contract("text"), Shape::Text).unwrap();
    let mut child = definition();
    child.nodes.get_mut("second").unwrap().activation = Activation::OnDemand;
    child.outputs.insert(
        "secret".into(),
        Port {
            contract: contract("text"),
            optional: false,
        },
    );
    child.results.insert(
        "secret".into(),
        Source::Constant {
            contract: contract("text"),
            value: json!("source content sk-fixture-secret"),
        },
    );
    let child = registry.compile(child).unwrap();
    let mut parent = definition();
    parent.nodes.clear();
    parent.outputs = child.artifact().definition.outputs.clone();
    parent.results = parent
        .compose(
            "nested",
            &child,
            BTreeMap::from([(
                "value".into(),
                Source::Constant {
                    contract: contract("integer"),
                    value: json!(783919),
                },
            )]),
        )
        .unwrap();
    let graph = Arc::new(registry.compile(parent).unwrap());
    let identity = graph.identity().to_string();
    let expected =
        serde_json::to_value(graph.inspection_definition(export_limits()).unwrap()).unwrap();
    let engine = Engine::new([graph]).unwrap();
    let checkpoint =
        Checkpoint::capture(&engine, &uuid::Uuid::new_v4().to_string(), limits()).unwrap();
    let bytes = checkpoint.bytes().to_vec();
    let snapshot = checkpoint
        .inspection_definition(&identity, export_limits())
        .unwrap();
    assert_eq!(serde_json::to_value(&snapshot).unwrap(), expected);
    assert_eq!(snapshot.artifact.definition.nodes.len(), 2);
    assert_eq!(
        snapshot.artifact.definition.nodes["nested/second"].activation,
        Activation::OnDemand
    );
    let serialized = serde_json::to_string(&snapshot).unwrap();
    assert!(!serialized.contains("sk-fixture-secret"));
    assert!(!serialized.contains("783919"));
    assert!(serialized.contains("Content"));
    assert_eq!(
        checkpoint
            .inspection_definition(
                &identity,
                ExportLimits {
                    bytes: 10,
                    attempts: 0
                }
            )
            .err()
            .unwrap()
            .code,
        "inspection_limit"
    );
    assert_eq!(checkpoint.bytes(), bytes);
    assert!(
        std::str::from_utf8(&bytes)
            .unwrap()
            .contains("sk-fixture-secret")
    );
}

#[test]
fn manifest_identity_mismatch_is_rejected_even_with_a_valid_envelope_checksum() {
    use sha2::{Digest, Sha256};
    let graph = Arc::new(registry().compile(definition()).unwrap());
    let engine = Engine::new([graph.clone()]).unwrap();
    let checkpoint =
        Checkpoint::capture(&engine, &uuid::Uuid::new_v4().to_string(), limits()).unwrap();
    // Exact format-1 field order. Recomputing the outer checksum must not let a
    // manifest label a different graph as the original artifact.
    let changed = BTreeMap::from([("incorrect-artifact-id", graph.artifact())]);
    let payload = format!(
        r#"{{"format":1,"engine":{},"artifacts":{},"events":[]}}"#,
        serde_json::to_string(&checkpoint.stamp().engine).unwrap(),
        serde_json::to_string(&changed).unwrap()
    );
    let checksum = format!("{:x}", Sha256::digest(payload.as_bytes()));
    let envelope = format!(r#"{{"payload":{payload},"checksum":"{checksum}"}}"#);
    assert_eq!(
        Checkpoint::decode(envelope.as_bytes(), limits())
            .err()
            .unwrap()
            .code,
        "checkpoint_artifact_mismatch"
    );
}
