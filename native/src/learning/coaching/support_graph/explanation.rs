//! Explicitly captured grammar-help evidence. A request snapshots available
//! learner evidence; it does not wait for a separate assessment to finish.
use super::*;
use crate::learning::practice::Presence;

#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Skill {
    pub id: String,
    pub name: String,
    pub overview: String,
}
#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Span {
    pub quote: String,
    pub start: usize,
    pub end: usize,
}
#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Evidence {
    pub source: SourceText,
    pub presence: BTreeMap<String, Presence>,
    // A missing entry means attribution is unavailable; an empty list means
    // attribution completed without locating a phrase. Preserve that distinction.
    pub spans: BTreeMap<String, Vec<Span>>,
}
#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Learning {
    pub skills: Vec<Skill>,
    pub evidence: Option<Evidence>,
}
#[derive(Clone)]
pub struct Request {
    pub exchange: super::Request,
    pub learning: Learning,
}
impl Request {
    pub fn schema(&self) -> serde_json::Value {
        support::schema(support::EXPLANATIONS)
    }
    pub fn text_request(
        &self,
        attempt: String,
        operation: String,
    ) -> crate::model::Result<TextRequest> {
        let mut captured = serde_json::json!(self.exchange.context);
        captured["presenceSkills"] = serde_json::json!(self.learning.skills);
        if let Some(evidence) = &self.learning.evidence {
            captured["skillAssessment"] = serde_json::json!({"presence":evidence.presence});
            let skills: BTreeMap<_, _> = evidence
                .spans
                .iter()
                .map(|(id, spans)| (id, serde_json::json!({"spans":spans})))
                .collect();
            captured["skillAttribution"] = serde_json::json!({"skills":skills});
        }
        let messages = support::prompt_for_exchange(
            self.exchange.source.text.clone(),
            self.exchange
                .learner
                .as_ref()
                .map(|source| source.text.clone()),
            support::EXPLANATIONS,
            &captured,
        )?;
        Ok(TextRequest {
            decisions: None,
            temperature: self.exchange.temperature,
            credential: self.exchange.target.credential.clone().unwrap_or_default(),
            model: self.exchange.target.model.clone(),
            route: self.exchange.target.route,
            target: self.exchange.target.clone(),
            install_id: self.exchange.install_id.clone(),
            attempt,
            operation,
            messages,
        })
    }
}
pub type Provider = Arc<
    dyn Fn(
            InvocationContext,
            Request,
        ) -> Pin<Box<dyn Future<Output = graph::Result<provider::Completion>> + Send>>
        + Send
        + Sync,
>;
pub fn operation_contract() -> Contract {
    Contract::new("conversation.explain-grammar", 1)
}
pub fn learning_contract() -> Contract {
    Contract::new("conversation.explanation-learning-snapshot", 1)
}
pub fn result_contract() -> Contract {
    Contract::new("conversation.bound-grammar-explanation", 1)
}
pub fn inputs() -> Ports {
    let mut ports = super::inputs();
    ports.insert("learning".into(), required(learning_contract()));
    ports
}
pub struct Captured {
    pub source: SourceText,
    pub learner: Option<SourceText>,
    pub context: Context,
    pub learning: Learning,
    pub target: ResolvedTarget,
    pub install_id: String,
}
pub fn capture(captured: Captured) -> graph::Result<Values> {
    let mut values = graph_text::capture(
        &captured.target,
        &captured.install_id,
        crate::ai::connections::model_routing::TASK_TEMPERATURE,
    )?;
    values.insert("source".into(), serde_json::json!(captured.source));
    if let Some(learner) = captured.learner {
        values.insert("learner".into(), serde_json::json!(learner));
    }
    values.insert("context".into(), serde_json::json!(captured.context));
    values.insert("learning".into(), serde_json::json!(captured.learning));
    decode(&values)?;
    Ok(values)
}
/// Explicit help requests capture their evidence at admission. This executable
/// is also the definition inspected for that request, with no synthetic edges to
/// assessment runs whose results were unavailable at capture time.
pub fn compile(provider: Provider) -> graph::Result<Executable> {
    let mut registry = Registry::default();
    graph_text::register_types(&mut registry)?;
    source_graph::register(&mut registry)?;
    contracts::register(&mut registry)?;
    register(&mut registry, provider)?;
    registry.compile(Definition {
        contract: Contract::new("conversation.grammar-help", 1),
        inputs: inputs(),
        outputs: BTreeMap::from([("support".into(), required(result_contract()))]),
        nodes: BTreeMap::from([(
            "explanations".into(),
            Node {
                operation: operation_contract(),
                inputs: inputs()
                    .keys()
                    .map(|name| (name.clone(), Source::Input(name.clone())))
                    .collect(),
                after: vec![],
                guard: None,
                activation: Activation::Automatic,
            },
        )]),
        results: BTreeMap::from([(
            "support".into(),
            Source::Output {
                node: "explanations".into(),
                port: "support".into(),
            },
        )]),
        compositions: BTreeMap::new(),
    })
}
pub fn decode(values: &Values) -> graph::Result<Request> {
    let exchange = decode_inputs(Task::Brief, values)?;
    let learning: Learning = serde_json::from_value(
        values
            .get("learning")
            .cloned()
            .ok_or_else(|| fault("learning"))?,
    )
    .map_err(|_| fault("learning"))?;
    validate_learning(exchange.learner.as_ref(), &learning)?;
    let request = Request { exchange, learning };
    request
        .text_request(String::new(), String::new())
        .map_err(|_| fault("prompt"))?;
    Ok(request)
}
pub(super) fn validate_learning(
    source: Option<&SourceText>,
    learning: &Learning,
) -> graph::Result<()> {
    let ids: std::collections::BTreeSet<_> = learning.skills.iter().map(|s| &s.id).collect();
    if ids.len() != learning.skills.len() || ids.iter().any(|id| id.is_empty()) {
        return Err(fault("learning.skills"));
    }
    if let Some(evidence) = &learning.evidence {
        if source.is_none_or(|source| {
            source.id != evidence.source.id || source.text != evidence.source.text
        }) || evidence.presence.keys().any(|id| !ids.contains(id))
            || evidence
                .spans
                .keys()
                .any(|id| !evidence.presence.contains_key(id))
        {
            return Err(fault("learning.source"));
        }
        let source: Vec<_> = evidence.source.text.encode_utf16().collect();
        use unicode_segmentation::UnicodeSegmentation;
        let boundaries: std::collections::BTreeSet<_> = evidence
            .source
            .text
            .grapheme_indices(true)
            .map(|(offset, _)| evidence.source.text[..offset].encode_utf16().count())
            .chain([source.len()])
            .collect();
        for spans in evidence.spans.values() {
            for span in spans {
                if span.start >= span.end
                    || span.end > source.len()
                    || !boundaries.contains(&span.start)
                    || !boundaries.contains(&span.end)
                    || source[span.start..span.end] != span.quote.encode_utf16().collect::<Vec<_>>()
                {
                    return Err(fault("learning.spans"));
                }
            }
        }
    }
    Ok(())
}
pub fn register(registry: &mut Registry, provider: Provider) -> graph::Result<()> {
    use super::contracts::{list, record};
    registry.define_type(
        learning_contract(),
        record([
            (
                "skills",
                list(record([
                    ("id", Shape::Text),
                    ("name", Shape::Text),
                    ("overview", Shape::Text),
                ])),
            ),
            (
                "evidence",
                Shape::Nullable(Box::new(record([
                    ("source", source_graph::shape()),
                    ("presence", Shape::Map(Box::new(Shape::Text))),
                    (
                        "spans",
                        Shape::Map(Box::new(list(record([
                            ("quote", Shape::Text),
                            ("start", Shape::Integer),
                            ("end", Shape::Integer),
                        ])))),
                    ),
                ]))),
            ),
        ]),
    )?;
    registry.define_type(
        result_contract(),
        record([
            ("source", source_graph::shape()),
            (
                "value",
                record([(
                    "cards",
                    list(record([
                        ("quote", Shape::Text),
                        ("title", Shape::Text),
                        ("body", Shape::Text),
                        ("example", Shape::Text),
                        ("contrast", Shape::Text),
                    ])),
                )]),
            ),
        ]),
    )?;
    registry.register(
        Operation {
            contract: operation_contract(),
            implementation: "learning/support/explanations/graph/1".into(),
            inputs: inputs(),
            outputs: BTreeMap::from([("support".into(), required(result_contract()))]),
            resource: Resource::Provider,
            reuse: Reuse::Exact,
        },
        Arc::new(move |invocation, values| {
            let provider = provider.clone();
            Box::pin(async move {
                let request = decode(&values)?;
                let completion = provider(invocation.clone(), request.clone()).await?;
                let value =
                    support::validate(support::EXPLANATIONS, &completion).map_err(|error| {
                        invocation
                            .observe(graph_evidence::failure(
                                &error,
                                &request.exchange.target.model,
                                &[&request.exchange.source.text, &completion.text],
                            ))
                            .err()
                            .unwrap_or_else(|| fault("response"))
                    })?;
                Ok(BTreeMap::from([(
                    "support".into(),
                    serde_json::json!({
                        "source":request.exchange.source, "value":value,
                    }),
                )]))
            })
        }),
    )
}
