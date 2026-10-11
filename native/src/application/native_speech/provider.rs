use super::*;

pub(super) async fn provide(
    state: &Arc<Application>,
    context: InvocationContext,
    request: synthesis::Request,
    live: std::sync::Weak<Producer>,
    pending: Arc<Mutex<Option<Payload>>>,
) -> crate::ai::graph::Result<synthesis::Receipt> {
    let mut key = Zeroizing::new(String::new());
    let result: Result<_> = async {
        let authorize =
            || state.speech_authority(&request.settings.target, &request.settings.install_id);
        authorize()?;
        {
            let store = state.lock()?;
            let owner = live.upgrade().ok_or_else(internal)?;
            let work = store
                .workspace_graphs
                .work(&store.connection, context.identity(), |run| {
                    run == owner.id()
                })
                .map_err(error)?;
            if work.operation != synthesis::operation_contract()
                || synthesis::decode(&work.inputs).map_err(error)?.source != request.source
            {
                return Err(internal());
            }
            graph_identity::load_workspace(&store.connection, context.identity())?;
        }
        if let Some(id) = &request.settings.target.credential {
            key = read_secret(id.clone()).await?;
        }
        authorize()?;
        if !live.upgrade().is_some_and(|p| p.has_subscribers()) {
            return Err(AppError::new(
                ErrorCode::Conflict,
                "Speech consumers closed before dispatch.",
            ));
        }
        let _permit = state
            .admission
            .try_chat()
            .ok_or_else(|| AppError::new(ErrorCode::AdmissionHeld, "Speech capacity is full."))?;
        let stream = graph_audio::producer_id(context.identity())?;
        state.lock()?.connection.execute(
            "UPDATE workspace_graph_consumers SET stream_id=?2 WHERE run_id=?1",
            rusqlite::params![live.upgrade().ok_or_else(internal)?.id(), stream],
        )?;
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
            let response = super::super::graph_speech::evidence(
                outcome,
                &request.settings.target.model,
                &[&key, &request.source.text],
            );
            let mut previous = observed.lock().map_err(|_| internal())?;
            if previous.as_ref() != Some(&response) {
                context.observe(response.clone()).map_err(error)?;
                *previous = Some(response);
                let mut guard = state.lock()?;
                if guard.snapshot()?.learner.id != request.settings.install_id {
                    return Err(AppError::new(
                        ErrorCode::SessionExpired,
                        "Workspace changed during speech.",
                    ));
                }
                let catalog: String = guard.connection.query_row(
                    "SELECT catalog FROM workspace_graph_engines WHERE id=?1",
                    [context.identity().engine.as_deref().ok_or_else(internal)?],
                    |r| r.get(0),
                )?;
                let store: &mut Store = &mut guard;
                store
                    .workspace_graphs
                    .event(
                        &mut store.connection,
                        &catalog,
                        Event::Observe(context.snapshot().map_err(error)?),
                        |_, _| Ok(()),
                    )
                    .map_err(error)?;
            }
            Ok(())
        };
        let client = crate::ai::transport::provider::client()?;
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
            state.content_writer(context.identity())?,
            transport,
        );
        tokio::pin!(transport);
        let completed = loop {
            tokio::select! {
                result = &mut transport => break result,
                _ = tokio::time::sleep(Duration::from_millis(50)) => authorize()?,
            }
        };
        context
            .observe(super::super::graph_speech::evidence(
                &completed,
                &request.settings.target.model,
                &[&key, &request.source.text],
            ))
            .map_err(error)?;
        let wav = completed.audio?;
        let receipt = graph_audio::reference(context.identity(), &wav)?;
        if completed
            .alignment
            .as_ref()
            .is_some_and(|a| !a.valid() || a.source_text != request.source.text)
        {
            return Err(AppError::new(
                ErrorCode::Validation,
                "Speech alignment differs from source.",
            ));
        }
        *pending.lock().map_err(|_| internal())? = Some(Payload {
            receipt: receipt.clone(),
            wav,
            alignment: completed.alignment,
            identity: Some(context.identity().clone()),
            request: request.clone(),
        });
        Ok(receipt)
    }
    .await;
    result.map_err(|cause| {
        context
            .observe(crate::ai::transport::graph_evidence::failure(
                &cause,
                &request.settings.target.model,
                &[&key, &request.source.text],
            ))
            .err()
            .unwrap_or_else(fault)
    })
}
