use super::*;
use serde_json::json;

fn settings() -> Settings {
    Settings {
        target_language: "spanish".into(),
        explanation_language: "english".into(),
        explanation_writing: vec![],
        romanization: vec![],
        segmentation: vec![],
        romanization_enabled: false,
    }
}
fn values() -> Values {
    capture(
        SourceText {
            id: "message".into(),
            text: "𐐀 sí sí!".into(),
        },
        settings(),
        &ResolvedTarget {
            audio_resolution: None,
            route: crate::model::ConnectionRoute::Custom,
            revision: 3,
            url: "http://127.0.0.1:12345/v1".into(),
            model: "model".into(),
            credential: None,
        },
        "install",
    )
    .unwrap()
}
fn completion(raw: &str) -> provider::Completion {
    provider::Completion {
        text: raw.into(),
        finish_reason: "stop".into(),
        actual_model: "served-model".into(),
        provider_id: "request".into(),
        input_tokens: Some(10),
        output_tokens: Some(5),
        diagnostics: None,
    }
}

#[test]
fn captured_settings_preserve_existing_prompt_and_recovery() {
    let registry = crate::configuration::Registry::bundled().unwrap();
    for language in ["spanish", "mandarin", "arabic"] {
        let context = registry
            .resolve_pair(language, None, "english", None)
            .unwrap();
        let source = SourceText {
            id: "message".into(),
            text: "e\u{301}你好 العربية".into(),
        };
        let identity = identity(&source, language, "english");
        let settings = Settings::from(&context);
        let before =
            adapter::build_word_gloss_prompt_with_context(&identity, &source.text, &context)
                .unwrap();
        let after = settings.prompt(&identity, &source.text).unwrap();
        assert_eq!(
            serde_json::to_value(before.messages).unwrap(),
            serde_json::to_value(after.messages).unwrap()
        );
        assert_eq!(before.output_schema, after.output_schema);
        let result = completion(r#"{"spans":[]}"#);
        let old = adapter::recovery::recover(&identity, &source.text, &result, &context).unwrap();
        let new =
            adapter::recovery::recover_with_settings(&identity, &source.text, &result, &settings)
                .unwrap();
        assert_eq!(old.analysis, new.analysis);
    }
}

#[tokio::test]
async fn demanded_gloss_recovers_independent_spans_and_preserves_readings_and_metadata() {
    let graph = Arc::new(compile(Arc::new(|invocation, request| Box::pin(async move {
        let wire = request.text_request("attempt".into(), "operation".into()).unwrap();
        assert_eq!(wire.attempt, "attempt");
        assert_eq!(wire.temperature, 0.7);
        assert_eq!(request.source.id, "message");
        let result = completion(r#"{"spans":[{"first":"g0002","last":"g0003","kind":"gloss","gloss":"yes","pronunciation":"see"},{"first":"g0005","last":"g0006","kind":"gloss","gloss":"indeed","romanization":"sí"},{"first":"private-invalid","last":"g0000","kind":"literal"}]}"#);
        invocation.observe(graph_evidence::completion(&result, &request.target.model, &[&request.source.text]))?;
        Ok(result)
    }))).unwrap());
    let mut engine = Engine::new([graph.clone()]).unwrap();
    engine
        .apply(Event::Begin {
            run: "run".into(),
            artifact: graph.identity().into(),
            inputs: values(),
            scope: "scope".into(),
            policy: BTreeMap::new(),
        })
        .unwrap();
    let capacity = Capacity {
        local: 2,
        provider: 2,
    };
    assert!(engine.apply(Event::Advance(capacity)).unwrap().is_empty());
    engine
        .apply(Event::Demand {
            run: "run".into(),
            node: "gloss".into(),
        })
        .unwrap();
    let work = engine.apply(Event::Advance(capacity)).unwrap();
    assert_eq!(work.len(), 1);
    let report = engine
        .claim(work[0].execution)
        .unwrap()
        .execute_with_provisional(
            EvidenceLimits {
                observations: 16,
                bytes: 16384,
            },
            ProvisionalLimits { bytes: 4096 },
        )
        .await;
    let analysis: result::Analysis =
        serde_json::from_value(report.outcome.as_ref().unwrap()["gloss"].clone()).unwrap();
    let view = analysis.view("operation", "attempt").unwrap();
    let words: Vec<_> = view
        .segments
        .iter()
        .filter(|s| s.kind == crate::model::GlossSegmentKind::Gloss)
        .collect();
    assert_eq!((words[0].start, words[0].end), (3, 5));
    assert_eq!((words[1].start, words[1].end), (6, 8));
    assert_eq!(words[0].pronunciation.as_deref(), Some("see"));
    assert_eq!(words[1].romanization.as_deref(), Some("sí"));
    assert_eq!(view.coverage, crate::model::GlossCoverage::Partial);
    let serialized = serde_json::to_string(&report).unwrap();
    assert!(!serialized.contains("private-invalid"));
    assert!(serialized.contains("gloss_unknown_boundary"));
    assert!(serialized.contains("served-model"));
    engine.apply(Event::SettleObserved(report)).unwrap();
    assert!(engine.outputs("run").unwrap().is_none());
    let attempt = engine.inspect("run").unwrap().attempts["gloss"]
        .last()
        .unwrap()
        .id;
    engine
        .apply(Event::Adopt {
            run: "run".into(),
            node: "gloss".into(),
            attempt,
        })
        .unwrap();
    assert_eq!(
        engine.outputs("run").unwrap().unwrap()["gloss"],
        json!(analysis)
    );
    assert_eq!(
        engine.inspect("run").unwrap().artifact.definition,
        graph.artifact().definition
    );
    assert!(engine.apply(Event::Advance(capacity)).unwrap().is_empty());
    let mut invalid = analysis.clone();
    invalid.pronunciations[0].start = 0;
    assert_eq!(invalid.validate().unwrap_err().path, "unbound_reading");
    let mut duplicate = analysis.clone();
    duplicate
        .pronunciations
        .push(duplicate.pronunciations[0].clone());
    assert_eq!(duplicate.validate().unwrap_err().path, "duplicate_reading");
}

#[tokio::test]
async fn malformed_envelope_fails_without_automatic_retry() {
    let graph = Arc::new(
        compile(Arc::new(|_, _| {
            Box::pin(async { Ok(completion("private malformed content")) })
        }))
        .unwrap(),
    );
    let mut engine = Engine::new([graph.clone()]).unwrap();
    engine
        .apply(Event::Begin {
            run: "run".into(),
            artifact: graph.identity().into(),
            inputs: values(),
            scope: "scope".into(),
            policy: BTreeMap::from([("gloss".into(), Activation::Automatic)]),
        })
        .unwrap();
    let capacity = Capacity {
        local: 2,
        provider: 2,
    };
    let work = engine.apply(Event::Advance(capacity)).unwrap();
    let report = engine
        .claim(work[0].execution)
        .unwrap()
        .execute_with_provisional(
            EvidenceLimits {
                observations: 16,
                bytes: 16384,
            },
            ProvisionalLimits { bytes: 4096 },
        )
        .await;
    assert!(report.outcome.is_err());
    let serialized = serde_json::to_string(&report).unwrap();
    assert!(!serialized.contains("private malformed content"));
    assert!(serialized.contains("word_gloss_validation"));
    engine.apply(Event::SettleObserved(report)).unwrap();
    assert!(engine.apply(Event::Advance(capacity)).unwrap().is_empty());
}
