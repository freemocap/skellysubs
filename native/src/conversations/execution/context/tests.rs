use super::*;
use graph::*;
use serde_json::json;

fn captured() -> Captured {
    Captured {
        kind: Kind::Reply,
        messages: vec![
            PromptMessage {
                role: "system".into(),
                content: "Instructions".into(),
            },
            PromptMessage {
                role: "assistant".into(),
                content: "العربية 日本語".into(),
            },
            PromptMessage {
                role: "user".into(),
                content: "  e\u{301} é 🦴\n".into(),
            },
        ],
        source_ids: vec!["retained-source".into()],
    }
}

fn executable() -> Arc<Executable> {
    let mut registry = Registry::default();
    register(&mut registry).unwrap();
    let ports = |contract| {
        BTreeMap::from([(
            "context".into(),
            Port {
                contract,
                optional: false,
            },
        )])
    };
    Arc::new(
        registry
            .compile(Definition {
                contract: Contract::new("context-test", 1),
                inputs: ports(captured_contract()),
                outputs: ports(validated_contract()),
                nodes: BTreeMap::from([(
                    "context".into(),
                    Node {
                        operation: operation_contract(),
                        inputs: BTreeMap::from([(
                            "context".into(),
                            Source::Input("context".into()),
                        )]),
                        after: vec![],
                        guard: None,
                        activation: Activation::Automatic,
                    },
                )]),
                results: BTreeMap::from([(
                    "context".into(),
                    Source::Output {
                        node: "context".into(),
                        port: "context".into(),
                    },
                )]),
                compositions: BTreeMap::new(),
            })
            .unwrap(),
    )
}

async fn execute(value: serde_json::Value) -> (Engine, InvocationReport) {
    let graph = executable();
    let mut engine = Engine::new([graph.clone()]).unwrap();
    engine
        .apply(Event::Begin {
            run: "run".into(),
            artifact: graph.identity().into(),
            inputs: BTreeMap::from([("context".into(), value)]),
            scope: "test-conversation".into(),
            policy: BTreeMap::new(),
        })
        .unwrap();
    let work = engine
        .apply(Event::Advance(Capacity {
            local: 1,
            provider: 0,
        }))
        .unwrap();
    assert_eq!(work.len(), 1);
    assert_eq!(work[0].resource, Resource::Local);
    let report = engine
        .claim(work[0].execution)
        .unwrap()
        .execute(EvidenceLimits {
            observations: 1,
            bytes: 1024,
        })
        .await;
    (engine, report)
}

#[tokio::test]
async fn registered_operation_preserves_source_bytes_and_requires_adoption() {
    let value = serde_json::to_value(captured()).unwrap();
    let (mut engine, report) = execute(value.clone()).await;
    assert_eq!(report.outcome.as_ref().unwrap()["context"], value);
    engine.apply(Event::SettleObserved(report)).unwrap();
    assert!(engine.outputs("run").unwrap().is_none());
    let attempt = engine.inspect("run").unwrap().attempts["context"][0].id;
    engine
        .apply(Event::Adopt {
            run: "run".into(),
            node: "context".into(),
            attempt,
        })
        .unwrap();
    assert_eq!(engine.outputs("run").unwrap().unwrap()["context"], value);
}

#[tokio::test]
async fn semantic_failures_never_create_validated_outputs_or_echo_content() {
    for (field, replacement, path) in [
        ("kind", json!("private invalid kind"), "context.kind"),
        (
            "messages",
            json!([{"role":"private role","content":"private prompt"}]),
            "context.messages.first.role",
        ),
    ] {
        let mut value = serde_json::to_value(captured()).unwrap();
        value[field] = replacement;
        let (mut engine, report) = execute(value).await;
        assert_eq!(
            report.outcome,
            Err(Fault {
                code: "conversation_context_invalid".into(),
                path: path.into(),
            })
        );
        engine.apply(Event::SettleObserved(report)).unwrap();
        assert_eq!(
            engine.disposition("run", "context").unwrap(),
            Disposition::Failed
        );
        assert!(engine.outputs("run").unwrap().is_none());
    }
}

#[test]
fn message_and_utf8_byte_limits_are_exact() {
    let mut context = captured();
    context.messages = vec![PromptMessage {
        role: "system".into(),
        content: String::new(),
    }];
    context.messages.extend((0..42).map(|_| PromptMessage {
        role: "user".into(),
        content: String::new(),
    }));
    context.messages[1].content = "é".repeat(48_000);
    assert_eq!(validate(&context), Ok(()));
    context.messages[1].content.push('x');
    assert_eq!(validate(&context), Err(Violation::ContentBytes));
    context.messages[1].content.clear();
    context.messages.push(PromptMessage {
        role: "user".into(),
        content: String::new(),
    });
    assert_eq!(validate(&context), Err(Violation::MessageCount));
}

#[test]
fn opening_and_reply_refinements_retain_existing_behavior() {
    let mut context = captured();
    context.kind = Kind::Opening;
    context.messages.truncate(1);
    assert_eq!(validate(&context), Err(Violation::OpeningSources));
    context.source_ids.clear();
    assert_eq!(validate(&context), Ok(()));
    context.kind = Kind::SeededOpening;
    assert_eq!(validate(&context), Err(Violation::OpeningMessages));
    context.messages.push(PromptMessage {
        role: "user".into(),
        content: "seed".into(),
    });
    assert_eq!(validate(&context), Ok(()));
    context.messages[1].role = "assistant".into();
    assert_eq!(validate(&context), Err(Violation::FinalUserMessage));
    context = captured();
    context.messages[1].role = "system".into();
    assert_eq!(validate(&context), Err(Violation::MessageRole));
}

#[test]
fn captured_context_cannot_bypass_refinement_through_identical_shapes() {
    let graph = executable();
    let mut definition = graph.artifact().definition.clone();
    definition
        .results
        .insert("context".into(), Source::Input("context".into()));
    let mut registry = Registry::default();
    register(&mut registry).unwrap();
    assert!(registry.compile(definition).is_err());
}
