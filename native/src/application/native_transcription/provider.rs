use super::*;
pub(super) async fn provide(
    state: &Arc<Application>,
    context: InvocationContext,
    values: Values,
) -> crate::ai::graph::Result<audio::TranscriptionResult> {
    let mut key = Zeroizing::new(String::new());
    let mut private = String::new();
    let mut model = String::new();
    let result:Result<_>=async {
        let owner={
            let store=state.lock()?;
            let hosts=state.native_transcription_hosts.lock().map_err(|_|internal())?;
            let mut selected=None;
            let work=store.workspace_graphs.work(&store.connection,context.identity(),|run| {
                if let Some(host)=hosts.get(run).and_then(std::sync::Weak::upgrade) {selected=Some(host);true} else {false}
            }).map_err(error)?;
            let owner=selected.ok_or_else(internal)?;
            if work.operation!=transcription::operation() || work.inputs!=values || transcription::capture(&owner.target,&owner.input,&owner.install)?!=values {return Err(internal());}
            graph_identity::load_workspace(&store.connection,context.identity())?;
            owner
        };
        private=owner.input.context.clone().unwrap_or_default();
        model=owner.target.model.clone();
        let validate=|| {
            state.transcription_authority(&owner.target,&owner.install)?;
            if !owner.live.upgrade().is_some_and(|p|p.has_subscribers()) {return Err(AppError::new(ErrorCode::Conflict,"Recording consumers closed before dispatch."));}
            Ok(())
        };
        let _permit=state.admission.audio(validate).await?;
        if let Some(id)=&owner.target.credential {key=read_secret(id.clone()).await?;}
        validate()?;
        let client=crate::ai::transport::provider::client()?;
        let transport=audio::transcribe(&client,&owner.target,&key,owner.input.clone(),&owner.install);
        let transport = crate::ai::content::capture::scope(state.content_writer(context.identity())?, transport);
        tokio::pin!(transport);
        let outcome=loop {tokio::select! {
            outcome=&mut transport=>break outcome?,
            _=tokio::time::sleep(Duration::from_millis(50))=>state.transcription_authority(&owner.target,&owner.install)?,
        }};
        context.observe(crate::ai::transport::graph_evidence::metadata(&json!({"requested_model":model,"diagnostics":outcome.diagnostics}),&[&key,&private,&outcome.result.text])).map_err(error)?;
        Ok(outcome.result)
    }.await;
    result.map_err(|cause| {
        context
            .observe(crate::ai::transport::graph_evidence::failure(
                &cause,
                &model,
                &[&key, &private],
            ))
            .err()
            .unwrap_or_else(fault)
    })
}
