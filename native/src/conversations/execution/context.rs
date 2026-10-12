//! Shared conversation-context refinement. Source authorization belongs to the
//! owner's transaction, not to this deterministic, reusable local operation.
use crate::ai::{graph, transport::provider::PromptMessage};
use serde::{Deserialize, Serialize};
use std::{collections::BTreeMap, sync::Arc};

#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Kind {
    Reply,
    Opening,
    SeededOpening,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct Captured {
    pub kind: Kind,
    pub messages: Vec<PromptMessage>,
    pub source_ids: Vec<String>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Violation {
    SystemMessage,
    FinalUserMessage,
    OpeningMessages,
    OpeningSources,
    MessageCount,
    MessageRole,
    ContentBytes,
}

impl Violation {
    fn fault(self) -> graph::Fault {
        let path = match self {
            Self::SystemMessage => "context.messages.first.role",
            Self::FinalUserMessage => "context.messages.last.role",
            Self::OpeningMessages | Self::MessageCount => "context.messages.length",
            Self::OpeningSources => "context.sourceIds",
            Self::MessageRole => "context.messages.role",
            Self::ContentBytes => "context.messages.content.bytes",
        };
        graph::Fault {
            code: "conversation_context_invalid".into(),
            path: path.into(),
        }
    }
}

/// Preserve the captured bytes and ordering. This checks prompt structure only;
/// a successful result does not establish access to the listed source records.
pub fn validate(context: &Captured) -> Result<(), Violation> {
    let messages = &context.messages;
    if messages.first().map(|m| m.role.as_str()) != Some("system") {
        return Err(Violation::SystemMessage);
    }
    match context.kind {
        Kind::Reply => {
            if messages.last().map(|m| m.role.as_str()) != Some("user") {
                return Err(Violation::FinalUserMessage);
            }
        }
        Kind::Opening | Kind::SeededOpening => {
            let expected = if matches!(context.kind, Kind::SeededOpening) {
                2
            } else {
                1
            };
            if messages.len() != expected {
                return Err(Violation::OpeningMessages);
            }
            if !context.source_ids.is_empty() {
                return Err(Violation::OpeningSources);
            }
            if expected == 2 && messages.last().map(|m| m.role.as_str()) != Some("user") {
                return Err(Violation::FinalUserMessage);
            }
        }
    }
    // System + optional history initialization + 40 sources + input.
    if messages.len() > 43 {
        return Err(Violation::MessageCount);
    }
    if messages
        .iter()
        .skip(1)
        .any(|m| m.role != "user" && m.role != "assistant")
    {
        return Err(Violation::MessageRole);
    }
    let bytes = messages
        .iter()
        .try_fold(0usize, |n, m| n.checked_add(m.content.len()));
    if bytes.is_none_or(|n| n > 96_000) {
        return Err(Violation::ContentBytes);
    }
    Ok(())
}

pub fn captured_contract() -> graph::Contract {
    graph::Contract::new("conversation.captured-context", 1)
}

pub fn validated_contract() -> graph::Contract {
    graph::Contract::new("conversation.validated-context", 1)
}

pub fn operation_contract() -> graph::Contract {
    graph::Contract::new("conversation.validate-context", 1)
}

/// Register once in the executable catalog. Persona and coach node occurrences
/// use this same operation; neither gets an independent validation implementation.
pub fn register(registry: &mut graph::Registry) -> graph::Result<()> {
    use graph::Shape;
    let shape = Shape::Record(BTreeMap::from([
        ("kind".into(), Shape::Text),
        ("sourceIds".into(), Shape::List(Box::new(Shape::Text))),
        (
            "messages".into(),
            Shape::List(Box::new(Shape::Record(BTreeMap::from([
                ("role".into(), Shape::Text),
                ("content".into(), Shape::Text),
            ])))),
        ),
    ]));
    registry.define_type(captured_contract(), shape.clone())?;
    registry.define_type(validated_contract(), shape)?;
    let port = |contract| {
        BTreeMap::from([(
            "context".into(),
            graph::Port {
                contract,
                optional: false,
            },
        )])
    };
    registry.register(
        graph::Operation {
            contract: operation_contract(),
            implementation: "conversations/execution/context/validate/1".into(),
            inputs: port(captured_contract()),
            outputs: port(validated_contract()),
            resource: graph::Resource::Local,
            reuse: graph::Reuse::Exact,
        },
        Arc::new(|_, values| {
            Box::pin(async move {
                let context: Captured =
                    serde_json::from_value(values["context"].clone()).map_err(|_| {
                        graph::Fault {
                            code: "conversation_context_invalid".into(),
                            path: "context.kind".into(),
                        }
                    })?;
                validate(&context).map_err(Violation::fault)?;
                Ok(values)
            })
        }),
    )
}

#[cfg(test)]
mod tests;
