//! Deferred application capabilities for the actual compiled partner artifacts.
//! Captured producer inputs are read by the host from native storage, never
//! reconstructed from UI state or a separate operation catalog.
use super::super::{context, partner_graph};
use super::*;
use crate::{
    ai::transport::provider::Completion, learning::coaching::support_graph::explanation_snapshot,
};
use std::{future::Future, pin::Pin};

pub type TextProvider = Arc<
    dyn Fn(
            InvocationContext,
        ) -> Pin<Box<dyn Future<Output = crate::ai::graph::Result<Completion>> + Send>>
        + Send
        + Sync,
>;
pub struct PartnerHost {
    pub reply: Arc<Executable>,
    pub opening: Arc<Executable>,
    pub playback: Arc<Executable>,
    pub gloss: Arc<Executable>,
    pub feedback: Arc<Executable>,
    text: Arc<OnceLock<TextProvider>>,
    evidence: Arc<OnceLock<explanation_snapshot::Reader>>,
    speech: Arc<OnceLock<crate::speech::synthesis_graph::Provider>>,
    lookup: Arc<OnceLock<crate::speech::synthesis_graph::playback::Lookup>>,
}
fn unbound() -> Fault {
    Fault {
        code: "provider_host_unbound".into(),
        path: "partner".into(),
    }
}
fn complete(
    binding: &Arc<OnceLock<TextProvider>>,
    context: InvocationContext,
) -> Pin<Box<dyn Future<Output = crate::ai::graph::Result<Completion>> + Send>> {
    let handler = binding.get().cloned();
    Box::pin(async move { handler.ok_or_else(unbound)?(context).await })
}
fn providers(
    text: &Arc<OnceLock<TextProvider>>,
    evidence: &Arc<OnceLock<explanation_snapshot::Reader>>,
    speech: &Arc<OnceLock<crate::speech::synthesis_graph::Provider>>,
    lookup: &Arc<OnceLock<crate::speech::synthesis_graph::playback::Lookup>>,
) -> partner_graph::Providers {
    let reply = text.clone();
    let translation = text.clone();
    let gloss = text.clone();
    let assessment = text.clone();
    let attribution = text.clone();
    let support = text.clone();
    let feedback = text.clone();
    let explanations = text.clone();
    let evidence = evidence.clone();
    let speech = speech.clone();
    let lookup = lookup.clone();
    partner_graph::Providers {
        speech: Arc::new(move |context, request| {
            let handler = speech.get().cloned();
            Box::pin(async move { handler.ok_or_else(unbound)?(context, request).await })
        }),
        audio_lookup: Arc::new(move |context, request| {
            let handler = lookup.get().cloned();
            Box::pin(async move { handler.ok_or_else(unbound)?(context, request).await })
        }),
        reply: Arc::new(move |context, request| {
            let result = complete(&reply, context.clone());
            Box::pin(async move {
                let completion = result.await?;
                prose::prepare_reply(&completion, &context).map_err(|cause| {
                    let mut private: Vec<&str> = request
                        .context
                        .messages
                        .iter()
                        .map(|m| m.content.as_str())
                        .collect();
                    private.push(&completion.text);
                    context
                        .observe(crate::ai::transport::graph_evidence::failure(
                            &cause,
                            &request.target.model,
                            &private,
                        ))
                        .err()
                        .unwrap_or(Fault {
                            code: "reply_response_invalid".into(),
                            path: "reply".into(),
                        })
                })
            })
        }),
        translation: Arc::new(move |context, _| complete(&translation, context)),
        gloss: Arc::new(move |context, _| complete(&gloss, context)),
        assessment: Arc::new(move |context, _| complete(&assessment, context)),
        attribution: Arc::new(move |context, _| complete(&attribution, context)),
        support: Arc::new(move |context, _| complete(&support, context)),
        feedback: Arc::new(move |context, _| complete(&feedback, context)),
        explanations: Arc::new(move |context, _| complete(&explanations, context)),
        available_evidence: Arc::new(move |context, source| {
            let reader = evidence.get().cloned();
            Box::pin(async move { reader.ok_or_else(unbound)?(context, source).await })
        }),
    }
}
impl PartnerHost {
    pub fn new() -> Result<Self> {
        let text = Arc::new(OnceLock::new());
        let evidence = Arc::new(OnceLock::new());
        let speech = Arc::new(OnceLock::new());
        let lookup = Arc::new(OnceLock::new());
        let bindings = providers(&text, &evidence, &speech, &lookup);
        let playback = Arc::new(
            crate::speech::synthesis_graph::playback::compile(
                bindings.speech,
                bindings.audio_lookup,
            )
            .map_err(error)?,
        );
        let gloss = Arc::new(
            crate::language::gloss_graph::compile(
                providers(&text, &evidence, &speech, &lookup).gloss,
            )
            .map_err(error)?,
        );
        let feedback = Arc::new(
            crate::learning::coaching::feedback_graph::compile(
                providers(&text, &evidence, &speech, &lookup).feedback,
            )
            .map_err(error)?,
        );
        Ok(Self {
            feedback,
            gloss,
            playback,
            reply: Arc::new(
                partner_graph::compile(
                    context::Kind::Reply,
                    providers(&text, &evidence, &speech, &lookup),
                )
                .map_err(error)?,
            ),
            opening: Arc::new(
                partner_graph::compile(
                    context::Kind::Opening,
                    providers(&text, &evidence, &speech, &lookup),
                )
                .map_err(error)?,
            ),
            text,
            evidence,
            speech,
            lookup,
        })
    }
    pub fn bind_speech(
        &self,
        speech: crate::speech::synthesis_graph::Provider,
        lookup: crate::speech::synthesis_graph::playback::Lookup,
    ) {
        self.speech.get_or_init(|| speech);
        self.lookup.get_or_init(|| lookup);
    }
    pub fn bind(&self, text: TextProvider, evidence: explanation_snapshot::Reader) {
        self.text.get_or_init(|| text);
        self.evidence.get_or_init(|| evidence);
    }
}
