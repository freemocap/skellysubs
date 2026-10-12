//! Native host for guide translation; the cache is an evictable product projection.
use super::*;
use crate::ai::{
    graph::*,
    transport::graph_identity,
    workspace_graph::{Progress, error},
};
use crate::language::reading::guide_graph;
use crate::model::Result;

pub(super) async fn produce(
    state: &Arc<Application>,
    request: &generation::Request,
    translation: Translation<'_>,
    producer: &results::pending::Producer<Retained>,
) -> Result<Retained> {
    let outcome = async {
        // Another opener may have finished between the command's cache read and
        // subscription. Recheck before admitting a fresh producer.
        {
            let store = state.lock()?;
            request.validate(&store)?;
            if let Some(saved) = cache::lookup(&store.connection, translation.key)? {
                return Ok(saved);
            }
        }
        let weak = Arc::downgrade(state);
        let graph = guide_graph::compile(Arc::new(move |invocation, _| {
            let state = weak.upgrade();
            Box::pin(async move {
                crate::application::workspace_text::provide(state.ok_or_else(|| Fault {code:"guide_owner_unavailable".into(),path:"owner".into()})?, invocation).await
            })
        })).map_err(error)?;
        let data = json!({"explanationLanguage":translation.explanation,"sourceExplanationLanguage":translation.edition.guide.explanation_language,"fields":translation.edition.fields(translation.variety),"context":translation.edition.guide.sections_for(translation.variety)});
        let messages = vec![provider::PromptMessage {role:"system".into(),content:translation.prompt.into()},provider::PromptMessage {role:"user".into(),content:data.to_string()}];
        let inputs = guide_graph::capture(request,messages,translation.edition.fields(translation.variety).len()).map_err(error)?;
        let catalog = {
            let mut guard = state.lock()?;
            request.validate(&guard)?;
            let store: &mut Store = &mut guard;
            store
                .workspace_graphs
                .begin(
                    &mut store.connection,
                    &request.install_id,
                    Arc::new(graph),
                    &request.id,
                    inputs,
                    &request.id,
                    "guide_translation",
                    &serde_json::json!({"guideKey":translation.key,"scope":{"language":request.language_id}}),
                    |_, _| Ok(()),
                )
                .map_err(error)?
        };
        let values = loop {
            let progress = {
                let mut guard = state.lock()?;
                let authority = request.validate(&guard);
                let store: &mut Store = &mut guard;
                if authority.is_err() || !producer.has_subscribers() {
                    store
                        .workspace_graphs
                        .event(
                            &mut store.connection,
                            &catalog,
                            Event::Cancel {
                                run: request.id.clone(),
                                node: None,
                            },
                            |_, _| Ok(()),
                        )
                        .map_err(error)?;
                    return Err(authority.err().unwrap_or_else(|| {
                        AppError::new(ErrorCode::Conflict, "Guide translation consumer closed.")
                    }));
                }
                let mut domain_error = None;
                store
                    .workspace_graphs
                    .poll(
                        &mut store.connection,
                        &catalog,
                        &request.id,
                        Capacity {
                            local: usize::MAX,
                            provider: admission::NETWORK_CAPACITY,
                        },
                        |db, event| {
                            if matches!(event.intent, CommitIntent::Dispatch { .. }) {
                                graph_identity::bind_workspace(db, event).map_err(|cause| {
                                    domain_error = Some(cause);
                                    Fault { code: "guide_wire_identity_failed".into(), path: "dispatch".into() }
                                })?;
                            }
                            Ok(())
                        },
                    )
                    .map_err(|fault| domain_error.unwrap_or_else(|| error(fault)))?
            };
            match progress {
                Progress::Waiting => tokio::time::sleep(Duration::from_millis(10)).await,
                Progress::Invoke(invocation) => {
                    let report = invocation
                        .execute(EvidenceLimits {
                            observations: 64,
                            bytes: 1024 * 1024,
                        })
                        .await;
                    let mut guard = state.lock()?;
                    if guard.learner_id()? != request.install_id {
                        return Err(AppError::new(
                            ErrorCode::SessionExpired,
                            "Workspace changed during guide generation.",
                        ));
                    }
                    let store: &mut Store = &mut guard;
                    store.workspace_graphs.event(&mut store.connection, &catalog, Event::Observe(EvidenceSnapshot {
                        identity: report.identity.clone(), observations: report.observations.clone(), evidence_failure: report.evidence_failure.clone(), provisional: report.provisional.clone(),
                    }), |_, _| Ok(())).map_err(error)?;
                    if let Err(fault) = store.workspace_graphs.event(&mut store.connection, &catalog, Event::SettleObserved(report), |_, _| Ok(())) {
                        let mut failure = error(fault);
                        let receipt = crate::ai::workspace_graph::receipt(&store.connection, &request.id)?;
                        failure.diagnostics = Some(serde_json::json!({"storage":failure.diagnostics,"response":receipt.map(|r| r["response"].clone())}));
                        return Err(failure);
                    }
                }
                Progress::Complete(values) => break values,
                Progress::Failed(cause) => {
                    let mut store = state.lock()?;
                    let (_,_,evidence)=store.workspace_graphs.result_record(&store.connection,&catalog,&request.id,"operation").map_err(error)?;
                    let receipt = crate::ai::workspace_graph::receipt(&store.connection, &request.id)?;
                    let domain=provider_failure(evidence,cause).with_diagnostics(serde_json::json!({"sourceExecution":receipt}));
                    generation::accept_completion(&mut store,request,&Err(domain.clone()))?;
                    return Err(domain);
                }
                Progress::Unknown => {
                    return Err(AppError::new(
                        ErrorCode::UnknownOutcome,
                        "Guide translation outcome is unknown. Start a new request explicitly.",
                    ));
                }
                Progress::Cancelled => {
                    return Err(AppError::new(
                        ErrorCode::Conflict,
                        "Guide translation was cancelled.",
                    ));
                }
                Progress::Held => {
                    return Err(AppError::new(
                        ErrorCode::AdmissionHeld,
                        "Guide translation execution is held.",
                    ));
                }
            }
        };
        let store = state.lock()?;
        request.validate(&store)?;
        let receipt = cache::receipt(&store.connection, &request.id)?;
        let markdown = translation.edition.translated(translation.variety, &json!({"texts":values.get("texts")}).to_string())?;
        let result = SkillGuideResult {markdown, explanation_language:translation.explanation.into(),generated:true,context:None,
            provenance:json!({"origin":"ai","review":"needs_review","sourcePaths":translation.edition.paths,"sourceFingerprint":translation.edition.fingerprint(),"sourceGuideRevision":translation.edition.guide.revision,"sourceSharedRevision":translation.edition.shared.revision,"model":receipt["response"]["actualModel"]})};
        let payload = serde_json::to_vec(&result)?;
        cache::retain(&store.connection, &request.id, translation.key, &payload)?;
        Ok(cache::retained(receipt, payload, false))
    }.await;
    if outcome.is_err() {
        let mut guard = state.lock()?;
        if guard.learner_id()? == request.install_id {
            let store: &mut Store = &mut guard;
            store
                .workspace_graphs
                .cancel_run(&mut store.connection, &request.id)
                .map_err(error)?;
        }
    }
    outcome
}
fn provider_failure(evidence: Option<ExecutionEvidence>, cause: Fault) -> AppError {
    evidence
        .and_then(|e| {
            e.observations.into_iter().rev().find_map(|o| {
                let code =
                    serde_json::from_value::<ErrorCode>(serde_json::json!(o.error_code?)).ok()?;
                let mut error = AppError::new(
                    code,
                    o.redacted_reason
                        .unwrap_or_else(|| "Guide translation failed.".into()),
                );
                if let Some(EvidenceValue::ClassifiedJson(transport)) =
                    o.additional.get("transport")
                {
                    error.refusal = transport
                        .get("refusal")
                        .and_then(|r| serde_json::from_value(r.clone()).ok());
                }
                Some(error)
            })
        })
        .unwrap_or_else(|| error(cause))
}
