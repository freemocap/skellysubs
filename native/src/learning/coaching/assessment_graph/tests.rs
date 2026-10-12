use super::*;
use serde_json::json;

fn fixture() -> (Values, serde_json::Value) {
    let registry = crate::configuration::Registry::bundled().unwrap();
    let skills: Vec<_> = registry
        .shared_skills()
        .skills
        .iter()
        .map(|s| SkillPrompt {
            id: s.id.clone(),
            name: s.name.clone(),
            overview: s.overview.clone(),
            boundary: s.boundary.clone(),
            language_guidance: "Captured shared guidance".into(),
        })
        .collect();
    let files = crate::configuration::content_files::read(
        &std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../content"),
    )
    .unwrap();
    let content = Content {
        skills: skills.clone(),
        instructions: crate::configuration::authoring::prompts::instructions(&files).unwrap(),
        questions: crate::configuration::authoring::prompts::message_questions(&files).unwrap(),
    };
    let source = SourceText {
        id: "learner-message".into(),
        text: "Exact learner source".into(),
    };
    let context = Context {
        messages: vec![
            provider::PromptMessage {
                role: "system".into(),
                content: "Instructions".into(),
            },
            provider::PromptMessage {
                role: "user".into(),
                content: source.text.clone(),
            },
        ],
        input: InputEvidence::default(),
        language: "captured language".into(),
        variety: "captured variety".into(),
    };
    let target = ResolvedTarget {
        audio_resolution: None,
        route: crate::model::ConnectionRoute::Custom,
        revision: 3,
        url: "http://127.0.0.1:12345/v1".into(),
        model: "ordinary-model".into(),
        credential: None,
    };
    let input = capture(source, context, content, &target, "install").unwrap();
    let mut raw = json!({});
    for skill in skills {
        raw[skill.id] = json!({"type":"choice","choice":"direct","confidence":0.875,"probabilities":{"absent":0.05,"contextual":0.05,"direct":0.85,"unclear":0.05}});
    }
    for (name, labels) in [
        ("grammar", turn_assessment::GRAMMAR_LABELS),
        (
            "understandability",
            turn_assessment::UNDERSTANDABILITY_LABELS,
        ),
    ] {
        let probabilities: BTreeMap<_, _> = labels.iter().map(|label| (*label, 0.25)).collect();
        raw[name] = json!({"type":"choice","choice":labels[0],"confidence":0.625,"probabilities":probabilities});
    }
    (input, raw)
}

#[tokio::test]
async fn native_assessment_preserves_distributions_and_source_without_publishing_credit() {
    let (input, raw) = fixture();
    let expected = raw.clone();
    let graph = Arc::new(
        compile(Arc::new(move |_, request| {
            let raw = raw.clone();
            Box::pin(async move {
                let wire = request
                    .text_request("attempt".into(), "operation".into())
                    .unwrap();
                assert_eq!(wire.model, assessment_adapter::MODEL);
                assert_eq!(
                    wire.decisions.as_ref().unwrap()["state"]["currentLearnerMessage"],
                    "Exact learner source"
                );
                assert_eq!(
                    wire.decisions.as_ref().unwrap()["questions"]
                        .as_object()
                        .unwrap()
                        .len(),
                    10
                );
                Ok(provider::Completion {
                    text: raw.to_string(),
                    finish_reason: "stop".into(),
                    actual_model: assessment_adapter::MODEL.into(),
                    provider_id: "provider-request".into(),
                    input_tokens: Some(13),
                    output_tokens: Some(7),
                    diagnostics: None,
                })
            })
        }))
        .unwrap(),
    );
    let mut engine = Engine::new([graph.clone()]).unwrap();
    engine
        .apply(Event::Begin {
            run: "run".into(),
            artifact: graph.identity().into(),
            inputs: input,
            scope: "source-authority".into(),
            policy: BTreeMap::new(),
        })
        .unwrap();
    let work = engine
        .apply(Event::Advance(Capacity {
            local: 1,
            provider: 1,
        }))
        .unwrap();
    let report = engine
        .claim(work[0].execution)
        .unwrap()
        .execute(EvidenceLimits {
            observations: 16,
            bytes: 16384,
        })
        .await;
    let output = &report.outcome.as_ref().unwrap()["assessment"];
    assert_eq!(output["source"]["id"], "learner-message");
    assert_eq!(output["assessment"]["grammar"]["confidence"], 0.625);
    for (id, value) in output["assessment"]["answers"].as_object().unwrap() {
        assert_eq!(value["probabilities"], expected[id]["probabilities"]);
        assert_eq!(value["confidence"], 0.875);
    }
    engine.apply(Event::SettleObserved(report)).unwrap();
    assert!(engine.outputs("run").unwrap().is_none());
    let attempt = engine.inspect("run").unwrap().attempts["assessment"]
        .last()
        .unwrap()
        .id;
    engine
        .apply(Event::Adopt {
            run: "run".into(),
            node: "assessment".into(),
            attempt,
        })
        .unwrap();
    assert!(engine.outputs("run").unwrap().is_some());
}

#[test]
fn capture_rejects_foreign_source_and_preserves_original_credit_policy() {
    let (mut input, mut raw) = fixture();
    let request = decode(&input).unwrap();
    let first = request.content.skills[0].id.clone();
    raw[&first]["probabilities"] =
        json!({"absent":0.8,"contextual":0.05,"direct":0.1,"unclear":0.05});
    let result = provider::Completion {
        text: raw.to_string(),
        finish_reason: "stop".into(),
        actual_model: "model".into(),
        provider_id: "request".into(),
        input_tokens: None,
        output_tokens: None,
        diagnostics: None,
    };
    let validated = assessment_adapter::validate_captured(
        &result,
        &request.content.skills,
        &request.content.instructions,
    )
    .unwrap();
    assert_eq!(validated["answers"][&first]["choice"], "direct");
    assert_eq!(validated["presence"][&first], "absent");
    input.get_mut("source").unwrap()["text"] = json!("foreign text");
    assert_eq!(decode(&input).err().unwrap().path, "source_or_model");
}
