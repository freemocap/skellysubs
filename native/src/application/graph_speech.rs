//! Speech transport for a native producer. Settlement retains its receipt and
//! evictable payload atomically with native execution ownership.
use super::*;
use crate::{
    ai::{graph, transport::graph_evidence},
    speech::synthesis_graph,
};

fn fault() -> graph::Fault {
    graph::Fault {
        code: "native_speech_failed".into(),
        path: "speech".into(),
    }
}
pub(super) fn evidence(
    outcome: &audio::SpeechOutcome,
    model: &str,
    private: &[&str],
) -> graph::ResponseEvidence {
    let mut response = graph_evidence::metadata(
        &serde_json::json!({
            "requested_model": model, "actual_model": outcome.actual_model,
            "request_id": outcome.provider_id, "finish_reason": outcome.finish_reason,
            "usage": {"input_tokens":outcome.input_tokens,"output_tokens":outcome.output_tokens},
            "cost_micros":outcome.cost_micros,"diagnostics":outcome.diagnostics,
        }),
        private,
    );
    response.usage = Some(graph::UsageEvidence {
        input_tokens: outcome.input_tokens,
        output_tokens: outcome.output_tokens,
        total_tokens: None,
        provenance: "provider_speech".into(),
    });
    response
}
async fn provide(
    state: Arc<Application>,
    client: reqwest::Client,
    invocation: graph::InvocationContext,
) -> graph::Result<synthesis_graph::Receipt> {
    let mut key = Zeroizing::new(String::new());
    let mut model = None;
    let mut source = String::new();
    let result: Result<_> = async {
        let authorize = || {
            let store = state.lock()?;
            store
                .graph_runtime
                .authorize_speech(&store.connection, invocation.identity())
        };
        let request = authorize()?;
        source = request.source.text.clone();
        model = Some(request.settings.target.model.clone());
        if let Some(credential) = &request.settings.target.credential {
            key = read_secret(credential.clone()).await?;
        }
        authorize()?;
        let stream = crate::speech::graph_audio::producer_id(invocation.identity())?;
        state
            .speech_streams
            .lock()
            .map_err(|_| internal())?
            .begin(&stream);
        let observed = Mutex::new(None);
        let emit = |pcm: &[u8],
                    alignment: Option<&crate::speech::alignment::SpeechAlignment>,
                    outcome: &audio::SpeechOutcome|
         -> Result<()> {
            authorize()?;
            state
                .speech_streams
                .lock()
                .map_err(|_| internal())?
                .append(&stream, pcm, alignment)?;
            let response = evidence(outcome, &request.settings.target.model, &[&key, &source]);
            let mut last = observed.lock().map_err(|_| internal())?;
            if last.as_ref() != Some(&response) {
                invocation
                    .observe(response.clone())
                    .map_err(execution::graph_runtime::error)?;
                *last = Some(response);
            }
            let mut guard = state.lock()?;
            let store: &mut Store = &mut guard;
            store.graph_runtime.observe(
                &mut store.connection,
                invocation
                    .snapshot()
                    .map_err(execution::graph_runtime::error)?,
            )
        };
        let input = request.speech_input();
        let transport = audio::synthesize_stream(
            &client,
            &request.settings.target,
            &key,
            &input,
            &request.settings.install_id,
            &emit,
        );
        let transport = crate::ai::content::capture::scope(
            state.content_writer(invocation.identity())?,
            transport,
        );
        tokio::pin!(transport);
        let completed = loop {
            tokio::select! {
                outcome = &mut transport => break outcome,
                _ = tokio::time::sleep(Duration::from_millis(250)) => { authorize()?; }
            }
        };
        invocation
            .observe(evidence(
                &completed,
                &request.settings.target.model,
                &[&key, &source],
            ))
            .map_err(execution::graph_runtime::error)?;
        let mut guard = state.lock()?;
        let store: &mut Store = &mut guard;
        store.graph_runtime.stage_audio(
            &store.connection,
            invocation.identity(),
            completed.audio?,
            completed.alignment,
        )
    }
    .await;
    result.map_err(|cause| {
        let mut response = graph_evidence::failure(
            &cause,
            model.as_deref().unwrap_or_default(),
            &[&key, &source],
        );
        response.requested_model = model;
        invocation.observe(response).err().unwrap_or_else(fault)
    })
}
pub(super) fn bind(state: &Arc<Application>, store: &Store, client: &reqwest::Client) {
    let speech = Arc::downgrade(state);
    let lookup = speech.clone();
    let client = client.clone();
    store.graph_runtime.partner.bind_speech(
        Arc::new(move |context, _| {
            let state = speech.upgrade();
            let client = client.clone();
            Box::pin(async move { provide(state.ok_or_else(fault)?, client, context).await })
        }),
        Arc::new(move |context, source| {
            let state = lookup.upgrade();
            Box::pin(async move {
                let result: Result<_> = (|| {
                    let state = state.ok_or_else(internal)?;
                    let mut guard = state.lock()?;
                    let store: &mut Store = &mut guard;
                    store
                        .graph_runtime
                        .lookup_audio(&store.connection, context.identity())
                })();
                result.map_err(|cause| {
                    let mut response = graph_evidence::failure(&cause, "", &[&source.source.text]);
                    response.requested_model = None;
                    context.observe(response).err().unwrap_or_else(fault)
                })
            })
        }),
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn native_speech_evidence_retains_partial_usage_and_billing_without_source_content() {
        let mut outcome = audio::SpeechOutcome::empty();
        outcome.provider_id = Some("provider-request".into());
        outcome.actual_model = Some("actual".into());
        outcome.input_tokens = Some(17);
        outcome.cost_micros = Some(83);
        outcome.diagnostics = Some(
            serde_json::json!({"status":502,"billing":{"cost_micros":83,"reported":true},"request_id":"provider-request","source_text":"private passage","credential":"private secret"}),
        );
        let response = evidence(
            &outcome,
            "requested",
            &["private passage", "private secret"],
        );
        assert_eq!(response.request_id.as_deref(), Some("provider-request"));
        assert_eq!(response.actual_model.as_deref(), Some("actual"));
        let usage = response.usage.as_ref().unwrap();
        assert_eq!(usage.input_tokens, Some(17));
        assert_eq!(usage.output_tokens, None);
        let encoded = serde_json::to_string(&response).unwrap();
        assert!(encoded.contains("cost_micros"));
        assert!(encoded.contains("83"));
        assert!(!encoded.contains("private passage"));
        assert!(!encoded.contains("private secret"));
    }
}
