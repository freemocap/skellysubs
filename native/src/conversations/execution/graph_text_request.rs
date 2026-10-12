//! Transport preparation from the exact native producer inputs. No source lookup,
//! route selection, scheduling or product publication occurs in this adapter.
use super::{graph_runtime, prose};
use crate::{
    ai::{
        graph::Work,
        transport::{graph_identity::WireIdentity, provider, text_request::TextRequest},
    },
    language::{gloss_graph, translation_graph},
    learning::coaching::{assessment_graph, attribution_graph, feedback_graph, support_graph},
    model::{AppError, ErrorCode, Result},
};
use serde_json::Value;

pub struct Prepared {
    pub request: TextRequest,
    schema: Option<Value>,
    name: &'static str,
    tokens: i32,
}
impl Prepared {
    pub fn output(&self) -> provider::RequestOutput<'_> {
        match &self.schema {
            Some(schema) => provider::RequestOutput::JsonSchema {
                name: self.name,
                schema,
                max_output_tokens: self.tokens,
            },
            None => provider::RequestOutput::Prose,
        }
    }
}
pub fn prepare(work: &Work, wire: WireIdentity) -> Result<Prepared> {
    let input = &work.inputs;
    let mut name = "coaching";
    let mut tokens = provider::MAX_OUTPUT_TOKENS;
    let (request, schema) = if work.operation == prose::operation_contract() {
        (
            prose::decode(input)
                .map_err(graph_runtime::error)?
                .text_request(wire.attempt, wire.operation),
            None,
        )
    } else if let Some(kind) = [
        crate::ai::generation::graph::Kind::Persona,
        crate::ai::generation::graph::Kind::Drill,
    ]
    .into_iter()
    .find(|kind| kind.operation() == work.operation)
    {
        let request =
            crate::ai::generation::graph::decode(kind, input).map_err(graph_runtime::error)?;
        tokens = request.max_output_tokens;
        let (request, schema, schema_name) = request.prepare(wire.attempt, wire.operation);
        name = schema_name;
        (request, Some(schema))
    } else if work.operation == crate::language::reading::guide_graph::operation() {
        let request =
            crate::language::reading::guide_graph::decode(input).map_err(graph_runtime::error)?;
        let (request, schema) = request.prepare(wire.attempt, wire.operation);
        name = "skill_guide_translation";
        tokens = 8192;
        (request, Some(schema))
    } else if work.operation == crate::language::reading::explanation_graph::operation() {
        let request = crate::language::reading::explanation_graph::decode(input)
            .map_err(graph_runtime::error)?;
        let (request, schema) = request.prepare(wire.attempt, wire.operation)?;
        (request, Some(schema))
    } else if work.operation == translation_graph::operation_contract() {
        let request = translation_graph::decode(input).map_err(graph_runtime::error)?;
        (
            request.text_request(wire.attempt, wire.operation),
            Some(request.schema()),
        )
    } else if work.operation == gloss_graph::operation_contract() {
        let request = gloss_graph::decode(input).map_err(graph_runtime::error)?;
        let prompt = request
            .prompt()
            .map_err(|_| crate::language::gloss::validation_error())?;
        name = prompt.format_id;
        tokens = provider::GLOSS_OUTPUT_TOKENS;
        (
            request
                .text_request(wire.attempt, wire.operation)
                .map_err(graph_runtime::error)?,
            Some(prompt.output_schema),
        )
    } else if work.operation == assessment_graph::operation_contract() {
        // The captured Jev request already carries its decision contract.
        let request = assessment_graph::decode(input).map_err(graph_runtime::error)?;
        (request.text_request(wire.attempt, wire.operation)?, None)
    } else if work.operation == attribution_graph::operation_contract() {
        let request = attribution_graph::decode(input).map_err(graph_runtime::error)?;
        (
            request.text_request(wire.attempt, wire.operation)?,
            Some(request.schema()),
        )
    } else if work.operation == feedback_graph::operation_contract() {
        let request = feedback_graph::decode(input).map_err(graph_runtime::error)?;
        tokens = 8192;
        (
            request.text_request(wire.attempt, wire.operation)?,
            Some(request.schema()?),
        )
    } else if work.operation == support_graph::explanation::operation_contract() {
        let request = support_graph::explanation::decode(input).map_err(graph_runtime::error)?;
        (
            request.text_request(wire.attempt, wire.operation)?,
            Some(request.schema()),
        )
    } else if let Some(task) = [support_graph::Task::Brief, support_graph::Task::Assistance]
        .into_iter()
        .find(|task| task.operation() == work.operation)
    {
        let request = support_graph::decode(task, input).map_err(graph_runtime::error)?;
        (
            request.text_request(wire.attempt, wire.operation)?,
            Some(request.schema()),
        )
    } else {
        return Err(AppError::new(
            ErrorCode::Conflict,
            "No text transport is registered for this native operation.",
        ));
    };
    Ok(Prepared {
        request,
        schema,
        name,
        tokens,
    })
}
