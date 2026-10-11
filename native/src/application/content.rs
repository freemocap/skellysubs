//! Bridge protected transport capture to the owned workspace. Each bounded write
//! is short; provider awaits and decoding never hold the application store lock.
use super::*;
impl Application {
    pub(crate) fn content_writer(
        self: &Arc<Self>,
        identity: &crate::ai::graph::InvocationIdentity,
    ) -> Result<crate::ai::content::capture::Writer> {
        let engine = identity.engine.clone().ok_or_else(internal)?;
        let execution = serde_json::to_string(&identity.execution)?;
        let state = self.clone();
        Ok(Arc::new(move |event| {
            let result = {
                let store = state.lock()?;
                crate::ai::content::write(
                    &store.connection,
                    &crate::ai::content::Owner::Execution {
                        engine: engine.clone(),
                        execution: execution.clone(),
                    },
                    event,
                )
            };
            if let Err(error) = &result
                && error.code != ErrorCode::Conflict
            {
                state.stop(error.clone());
            }
            result
        }))
    }
}

#[tauri::command]
pub(crate) async fn read_original_content(
    state: tauri::State<'_, Arc<Application>>,
    id: String,
) -> Result<crate::ai::content::ContentPayload> {
    let state = state.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        let prepared = crate::ai::content::prepare(&state.lock()?.connection, &id)?;
        prepared.finish()
    })
    .await
    .map_err(|cause| {
        crate::diagnostics::failures::join(&cause, "original_content_read", internal())
    })?
}

#[tauri::command]
pub(crate) async fn list_original_content(
    state: tauri::State<'_, Arc<Application>>,
    engine: String,
    execution: String,
) -> Result<Vec<crate::ai::content::ContentRecord>> {
    crate::ai::content::for_execution(&state.lock()?.connection, &engine, &execution)
}
#[tauri::command]
pub(crate) async fn recording_original_content(
    state: tauri::State<'_, Arc<Application>>,
    recording: String,
) -> Result<Vec<crate::ai::content::ContentRecord>> {
    let state = state.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        use rusqlite::OptionalExtension;
        let (mut records,prepared)={
            let store=state.lock()?;
            let records=crate::ai::content::for_recording(&store.connection,&recording)?;
            let parent:Option<(String,String)>=store.connection.query_row("SELECT r.engine_id,r.run_id FROM workspace_graph_consumers c JOIN workspace_graph_runs r ON r.run_id=c.run_id WHERE c.consumer_id=?1",[&recording],|r|Ok((r.get(0)?,r.get(1)?))).optional()?;
            let prepared=parent.map(|(engine,run)| Ok::<_,AppError>((crate::ai::inspection::prepare(&store.connection,&engine,&run)?,engine,run))).transpose()?;
            (records,prepared)
        };
        if let Some((prepared,engine,run))=prepared {
            let key=serde_json::to_string(&("recording_sources",prepared.cache_identity()?,&run))?;
            let history=state.inspect_cached(key,||prepared.history(&run,None,1))?;
            let executions:std::collections::BTreeSet<_>=history.frames.iter().flat_map(|frame|frame.attempts.values()).flatten().map(|attempt|attempt.execution.as_str()).collect();
            let store=state.lock()?;
            for execution in executions {records.extend(crate::ai::content::for_execution(&store.connection,&engine,execution)?);}
        }
        Ok(records)
    }).await.map_err(|cause|crate::diagnostics::failures::join(&cause,"recording_content_read",internal()))?
}
