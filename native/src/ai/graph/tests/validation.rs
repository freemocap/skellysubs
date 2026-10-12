use super::*;

#[test]
fn compilation_and_inspection_retain_the_actual_bound_structure() {
    let registry = registry();
    let definition = definition();
    let graph = Arc::new(registry.compile(definition.clone()).unwrap());
    let mut engine = Engine::new([graph.clone()]).unwrap();
    begin(&mut engine, &graph, "run", 1);
    let view = engine.inspect("run").unwrap();
    assert!(std::ptr::eq(view.artifact, graph.artifact()));
    assert_eq!(view.artifact.definition, definition);
    assert_eq!(view.nodes.len(), 2);
    let serialized = serde_json::to_value(&view).unwrap();
    assert!(serialized.get("inputs").is_none());
    assert_eq!(serialized["artifact_id"], graph.identity());
    let restored: Artifact =
        serde_json::from_value(serde_json::to_value(graph.artifact()).unwrap()).unwrap();
    assert_eq!(&restored, graph.artifact());
}

#[test]
fn graph_identity_changes_with_actual_bindings_and_implementation_versions() {
    let mut r = registry();
    let a = r.compile(definition()).unwrap();
    let mut altered = definition();
    altered
        .nodes
        .get_mut("second")
        .unwrap()
        .inputs
        .insert("value".into(), Source::Input("value".into()));
    assert_ne!(a.identity(), r.compile(altered).unwrap().identity());
    // Simulate another deployed implementation registration before compilation.
    r.operations
        .get_mut(&contract("increment"))
        .unwrap()
        .0
        .implementation = "synthetic/increment/v2".into();
    assert_ne!(a.identity(), r.compile(definition()).unwrap().identity());
    assert_eq!(
        a.artifact().operations[&contract("increment")].implementation,
        "synthetic/increment/v1"
    );
}

#[test]
fn rejects_missing_ports_cycles_guards_and_implicit_coercion() {
    let r = registry();
    let mut d = definition();
    d.nodes.get_mut("first").unwrap().inputs.clear();
    assert_eq!(r.compile(d).err().unwrap().code, "binding_set");
    let mut d = definition();
    d.nodes
        .get_mut("first")
        .unwrap()
        .inputs
        .insert("value".into(), source("second"));
    assert_eq!(r.compile(d).err().unwrap().code, "dependency_cycle");
    let mut d = definition();
    d.nodes.get_mut("first").unwrap().after = vec!["absent".into()];
    assert_eq!(r.compile(d).err().unwrap().code, "unknown_control_parent");
    let mut d = definition();
    d.nodes.get_mut("first").unwrap().inputs.insert(
        "value".into(),
        Source::Constant {
            contract: contract("boolean"),
            value: json!(true),
        },
    );
    assert_eq!(r.compile(d).err().unwrap().code, "incompatible_port");
    let mut d = definition();
    d.nodes.get_mut("second").unwrap().guard = Some(Source::Input("value".into()));
    // Graph output is also invalid because a guarded node may be absent.
    d.outputs.get_mut("value").unwrap().optional = true;
    assert_eq!(r.compile(d).err().unwrap().code, "invalid_guard");
}

#[test]
fn duplicate_registrations_are_rejected_without_replacing_handlers() {
    let mut r = registry();
    assert_eq!(
        r.define_type(contract("integer"), Shape::Text)
            .unwrap_err()
            .code,
        "duplicate_type"
    );
    let (op, handler) = r.operations[&contract("increment")].clone();
    assert_eq!(
        r.register(op, handler).unwrap_err().code,
        "duplicate_operation"
    );
    assert!(r.compile(definition()).is_ok());
}

#[test]
fn composition_preserves_boundaries_namespaces_and_identity_atomically() {
    let r = registry();
    let child = r.compile(definition()).unwrap();
    let mut parent = definition();
    parent.nodes.clear();
    parent.results.clear();
    let results = parent
        .compose(
            "nested",
            &child,
            BTreeMap::from([("value".into(), Source::Input("value".into()))]),
        )
        .unwrap();
    parent.results = results;
    let graph = r.compile(parent.clone()).unwrap();
    assert_eq!(graph.artifact().definition.nodes.len(), 2);
    assert_eq!(
        graph.artifact().definition.compositions["nested"].artifact,
        child.identity()
    );
    assert_eq!(
        graph.artifact().definition.nodes["nested/second"].inputs["value"],
        source("nested/first")
    );
    let before = parent.clone();
    assert!(
        parent
            .compose(
                "nested",
                &child,
                BTreeMap::from([("value".into(), Source::Input("value".into()))])
            )
            .is_err()
    );
    assert_eq!(parent, before);
    let mut wrong = definition();
    wrong.nodes.clear();
    wrong.results.clear();
    wrong.results = wrong
        .compose(
            "nested",
            &child,
            BTreeMap::from([("value".into(), Source::Absent(contract("integer")))]),
        )
        .unwrap();
    assert_eq!(r.compile(wrong).err().unwrap().code, "incompatible_port");
}

#[test]
fn control_and_guard_dependencies_participate_in_cycle_detection() {
    let r = registry();
    let mut d = definition();
    d.nodes
        .get_mut("first")
        .unwrap()
        .after
        .push("second".into());
    assert_eq!(r.compile(d).err().unwrap().code, "dependency_cycle");
    let mut d = definition();
    d.nodes.get_mut("second").unwrap().after = vec!["first".into(), "first".into()];
    assert_eq!(r.compile(d).err().unwrap().code, "duplicate_control");
}
