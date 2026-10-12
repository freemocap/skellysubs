//! Skill attribution consumes the adopted assessment. Selection is an explicit
//! local node; empty selection skips the provider through a native boolean guard.
mod contracts;
use super::{assessment_graph, skill_attribution};
use crate::{
    ai::{
        connections::access::ResolvedTarget,
        graph::{self, *},
        transport::{graph_evidence, graph_text, provider, text_request::TextRequest},
    },
    language::source_graph::{self, SourceText},
};
use serde::{Deserialize, Serialize};
use std::{
    collections::{BTreeMap, BTreeSet},
    future::Future,
    pin::Pin,
    sync::Arc,
};

#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Selection {
    pub source: SourceText,
    pub selected: Vec<String>,
}
impl Selection {
    pub fn ids(&self) -> BTreeSet<String> {
        self.selected.iter().cloned().collect()
    }
}
#[derive(Clone)]
pub struct Request {
    pub selection: Selection,
    pub content: assessment_graph::Content,
    pub target: ResolvedTarget,
    pub install_id: String,
    pub temperature: f64,
}
impl Request {
    pub fn text_request(
        &self,
        attempt: String,
        operation: String,
    ) -> crate::model::Result<TextRequest> {
        let messages = skill_attribution::prompt_for_selection(
            &self.selection.source.text,
            &self.selection.ids(),
            &self.content.skills,
            &self.content.instructions.attribution,
        )?;
        Ok(TextRequest {
            decisions: None,
            temperature: self.temperature,
            credential: self.target.credential.clone().unwrap_or_default(),
            model: self.target.model.clone(),
            route: self.target.route,
            target: self.target.clone(),
            install_id: self.install_id.clone(),
            attempt,
            operation,
            messages,
        })
    }
    pub fn schema(&self) -> serde_json::Value {
        skill_attribution::schema(&self.selection.ids())
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
fn fault(path: &str) -> Fault {
    Fault {
        code: "attribution_graph_invalid".into(),
        path: path.into(),
    }
}
pub fn selection_operation() -> Contract {
    Contract::new("learning.select-attribution", 1)
}
pub fn operation_contract() -> Contract {
    Contract::new("learning.attribute-skills", 1)
}
pub fn selection_contract() -> Contract {
    Contract::new("learning.attribution-selection", 1)
}
pub fn enabled_contract() -> Contract {
    Contract::new("learning.has-attribution-targets", 1)
}
pub fn result_contract() -> Contract {
    Contract::new("learning.bound-attribution", 1)
}
fn required(contract: Contract) -> Port {
    Port {
        contract,
        optional: false,
    }
}
pub fn inputs() -> Ports {
    let mut inputs = graph_text::inputs();
    inputs.insert("selection".into(), required(selection_contract()));
    inputs.insert(
        "content".into(),
        Port {
            contract: assessment_graph::content_contract(),
            optional: true,
        },
    );
    inputs
}
pub fn decode(values: &Values) -> graph::Result<Request> {
    let settings = graph_text::decode(values)?;
    let selection: Selection = serde_json::from_value(
        values
            .get("selection")
            .cloned()
            .ok_or_else(|| fault("selection"))?,
    )
    .map_err(|_| fault("selection"))?;
    if selection.source.id.is_empty() || selection.selected.len() != selection.ids().len() {
        return Err(fault("selection"));
    }
    let content = serde_json::from_value(
        values
            .get("content")
            .cloned()
            .ok_or_else(|| fault("content"))?,
    )
    .map_err(|_| fault("content"))?;
    let request = Request {
        selection,
        content,
        target: settings.target,
        install_id: settings.install_id,
        temperature: settings.temperature,
    };
    request
        .text_request(String::new(), String::new())
        .map_err(|_| fault("request"))?;
    Ok(request)
}

/// Assessment, source and transport types are supplied by the enclosing graph.
pub fn register(registry: &mut Registry, provider: Provider) -> graph::Result<()> {
    contracts::register(registry)?;
    registry.register(
        Operation {
            contract: selection_operation(),
            implementation: "learning/attribution/selection/1".into(),
            inputs: BTreeMap::from([(
                "assessment".into(),
                required(assessment_graph::result_contract()),
            )]),
            outputs: BTreeMap::from([
                ("selection".into(), required(selection_contract())),
                ("enabled".into(), required(enabled_contract())),
            ]),
            resource: Resource::Local,
            reuse: Reuse::Exact,
        },
        Arc::new(|_, values| {
            Box::pin(async move {
                let assessment = &values["assessment"];
                let source: SourceText = serde_json::from_value(assessment["source"].clone())
                    .map_err(|_| fault("source"))?;
                let selected: Vec<_> = skill_attribution::selected(
                    &serde_json::json!({"skillAssessment":assessment["assessment"]}),
                )
                .map_err(|_| fault("assessment"))?
                .into_iter()
                .collect();
                let enabled = !selected.is_empty();
                let selection = Selection { source, selected };
                Ok(BTreeMap::from([
                    ("selection".into(), serde_json::json!(selection)),
                    ("enabled".into(), serde_json::json!(enabled)),
                ]))
            })
        }),
    )?;
    registry.register(Operation {
        contract:operation_contract(), implementation:"learning/attribution/graph/1".into(), inputs:inputs(),
        outputs:BTreeMap::from([("attribution".into(), required(result_contract()))]), resource:Resource::Provider, reuse:Reuse::Exact,
    }, Arc::new(move |invocation, values| {
        let provider = provider.clone();
        Box::pin(async move {
            let request = decode(&values)?;
            let completion = provider(invocation.clone(), request.clone()).await?;
            let attribution = skill_attribution::validate_source(&request.selection.source.text, &request.selection.ids(), &completion)
                .map_err(|error| invocation.observe(graph_evidence::failure(&error, &request.target.model, &[&request.selection.source.text, &completion.text])).err().unwrap_or_else(|| fault("response")))?;
            Ok(BTreeMap::from([("attribution".into(), serde_json::json!({"source":request.selection.source,"attribution":attribution}))]))
        })
    }))
}
