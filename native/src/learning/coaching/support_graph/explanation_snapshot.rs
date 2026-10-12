//! Explicit local read at the time grammar help is requested. This is a fresh
//! operation, not a dependency on eventual completion of assessment/attribution.
use super::{explanation, *};

pub type Reader = Arc<
    dyn Fn(
            InvocationContext,
            SourceText,
        )
            -> Pin<Box<dyn Future<Output = graph::Result<Option<explanation::Evidence>>> + Send>>
        + Send
        + Sync,
>;
pub fn operation_contract() -> Contract {
    Contract::new("learning.snapshot-available-evidence", 1)
}
pub fn skills_contract() -> Contract {
    Contract::new("learning.explanation-skill-definitions", 1)
}
pub fn inputs() -> Ports {
    BTreeMap::from([
        (
            "source".into(),
            Port {
                contract: source_graph::contract(),
                optional: true,
            },
        ),
        (
            "skills".into(),
            Port {
                contract: skills_contract(),
                optional: true,
            },
        ),
    ])
}
pub fn register(registry: &mut Registry, reader: Reader) -> graph::Result<()> {
    use super::contracts::{list, record};
    registry.define_type(
        skills_contract(),
        list(record([
            ("id", Shape::Text),
            ("name", Shape::Text),
            ("overview", Shape::Text),
        ])),
    )?;
    registry.register(
        Operation {
            contract: operation_contract(),
            implementation: "learning/available-evidence/snapshot/1".into(),
            inputs: inputs(),
            outputs: BTreeMap::from([(
                "learning".into(),
                required(explanation::learning_contract()),
            )]),
            resource: Resource::Local,
            reuse: Reuse::Fresh,
        },
        Arc::new(move |invocation, values| {
            let reader = reader.clone();
            Box::pin(async move {
                let source: Option<SourceText> = values
                    .get("source")
                    .cloned()
                    .map(serde_json::from_value)
                    .transpose()
                    .map_err(|_| fault("source"))?;
                let skills: Vec<explanation::Skill> = serde_json::from_value(
                    values
                        .get("skills")
                        .cloned()
                        .ok_or_else(|| fault("skills"))?,
                )
                .map_err(|_| fault("skills"))?;
                let evidence = match source.clone() {
                    Some(source) => {
                        if source.id.is_empty() || source.text.trim().is_empty() {
                            return Err(fault("source"));
                        }
                        reader(invocation, source).await?
                    }
                    None => None,
                };
                let learning = explanation::Learning { skills, evidence };
                explanation::validate_learning(source.as_ref(), &learning)?;
                Ok(BTreeMap::from([(
                    "learning".into(),
                    serde_json::json!(learning),
                )]))
            })
        }),
    )
}
