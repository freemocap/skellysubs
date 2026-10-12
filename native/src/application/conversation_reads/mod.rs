//! Committed conversation display hydration and workspace-lifetime reuse.
use super::*;
mod cache;
use super::workspace_version::Version;
pub(super) use cache::Cache;
use cache::Prepared;

impl Application {
    pub(super) fn execute_command(&self, command: Command) -> Result<Receipt> {
        let navigation = matches!(command.action, model::Action::OpenConversation { .. });
        let mut store = self.lock()?;
        let before = Version::of(&store);
        let receipt = store.execute(command)?;
        if navigation {
            self.learning_reads
                .lock()
                .map_err(|_| internal())?
                .navigation_committed(&before, &Version::of(&store));
            self.scheduler_idle
                .lock()
                .map_err(|_| internal())?
                .navigation_committed(&before, &Version::of(&store));
            self.conversation_reads
                .lock()
                .map_err(|_| internal())?
                .navigation_committed(&before, &Version::of(&store), receipt.revision);
        }
        Ok(receipt)
    }
}

pub(super) async fn read_conversation_since(
    state: Arc<Application>,
    conversation: String,
    before: Option<i32>,
    revision: i32,
    force: bool,
) -> Result<Option<Arc<Prepared>>> {
    tauri::async_runtime::spawn_blocking(move || {
        for _ in 0..3 {
            let sources = {
                let store = state.lock()?;
                if let Some(cached) = state.conversation_reads.lock().map_err(|_| internal())?.get(
                    &Version::of(&store), &conversation, before,
                ) {
                    return Ok(if !force && cached.revision == revision { None } else { Some(cached) });
                }
                let current: i32 =
                    store
                        .connection
                        .query_row("SELECT revision FROM metadata", [], |r| r.get(0))?;
                if !force && current == revision {
                    return Ok(None);
                }
                store
                    .graph_runtime
                    .detached_views(&store.connection, &conversation)?
            };
            let mut stamps = Vec::new();
            let mut views = std::collections::BTreeMap::new();
            for source in sources {
                let stamp = source.source.source.stamp().clone();
                let key = serde_json::to_string(&(
                    "conversation_read",
                    source.source.cache_identity()?,
                    &source.runs,
                ))?;
                let snapshots =
                    state.inspect_cached(key, || source.source.current_snapshots(&source.runs))?;
                views.insert(stamp.engine.clone(), snapshots);
                stamps.push(stamp);
            }
            let mut guard = state.lock()?;
            let store: &mut Store = &mut guard;
            if !store
                .graph_runtime
                .install_read_views(&store.connection, &stamps, views)?
            {
                continue;
            }
            let result = (|| {
                use rusqlite::OptionalExtension;
                let current:i32=store.connection.query_row("SELECT revision FROM metadata",[],|r|r.get(0))?;
                if !force && current==revision { return Ok(None); }
                let snapshot=store.conversation_snapshot_receipts(&conversation,before,false)?;
                let mut receipts=Vec::new();
                for recording in &snapshot.transcription_attempts {
                    let source:Option<(String,String,Option<String>)>=store.connection.query_row("SELECT r.engine_id,r.run_id,a.execution_id FROM workspace_graph_consumers c JOIN workspace_graph_runs r ON r.run_id=c.run_id LEFT JOIN native_graph_audio_receipts a ON a.id=c.stream_id WHERE c.consumer_id=?1",[&recording.id],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?))).optional()?;
                    if let Some((engine,run,execution))=source {
                        let source=crate::ai::inspection::prepare(&store.connection,&engine,&run)?;
                        receipts.push((recording.id.clone(),engine,run,execution,source));
                    }
                }
                Ok::<_,AppError>(Some((snapshot,receipts,Version::of(store))))
            })();
            store.graph_runtime.clear_read_views();
            drop(guard);
            let Some((mut snapshot,receipts,version))=result? else {return Ok(None)};
            for (id,engine,run,execution,source) in receipts {
                let key=serde_json::to_string(&("recording_receipt",source.cache_identity()?,&run,&execution))?;
                let receipt=state.inspect_cached(key,||source.workspace_receipt(&engine,&run,execution.map(|v|serde_json::from_str(&v)).transpose()?))?;
                if let Some(receipt)=receipt {
                    let recording=snapshot.transcription_attempts.iter_mut().find(|r|r.id==id).ok_or_else(internal)?;
                    recording.diagnostics.get_or_insert_with(||serde_json::json!({}))["sourceExecution"]=receipt;
                }
            }
            let prepared = Prepared::new(&snapshot)?;
            // Receipt rendering and JSON encoding happen outside the store lock.
            // A concurrent write may make this response old, but it must never
            // install an old result as the next reader's current working set.
            remember(&state, &version, conversation, before, prepared.clone())?;
            return Ok(Some(prepared));
        }
        Err(AppError::new(
            ErrorCode::Conflict,
            "Conversation changed during inspection. Refresh the view.",
        ))
    })
    .await
    .map_err(|cause| {
        crate::diagnostics::failures::join(&cause, "conversation_read_worker", internal())
    })?
}

fn remember(
    state: &Application,
    version: &Version,
    conversation: String,
    before: Option<i32>,
    prepared: Arc<Prepared>,
) -> Result<()> {
    let store = state.lock()?;
    if Version::of(&store) == *version {
        state
            .conversation_reads
            .lock()
            .map_err(|_| internal())?
            .insert(version, conversation, before, prepared);
    }
    Ok(())
}

#[cfg(test)]
mod tests;

#[cfg(test)]
mod contention_tests {
    use super::*;

    #[tokio::test(flavor = "current_thread")]
    async fn waiting_conversation_read_does_not_block_async_progress() {
        let directory = tempfile::tempdir().unwrap();
        let state = Application::recording_fixture(&directory.path().join("workspace.sqlite3"));
        let owner = state.clone();
        let (locked, ready) = std::sync::mpsc::channel();
        let (release, released) = std::sync::mpsc::channel();
        let holder = std::thread::spawn(move || {
            let _guard = owner.lock().unwrap();
            locked.send(()).unwrap();
            released.recv_timeout(Duration::from_secs(2)).is_ok()
        });
        ready.recv().unwrap();
        let read = tokio::spawn(read_conversation_since(
            state,
            "missing".into(),
            None,
            -1,
            true,
        ));
        tokio::time::sleep(Duration::from_millis(10)).await;
        let _ = release.send(());
        assert!(
            holder.join().unwrap(),
            "store wait blocked the async worker until the failsafe expired"
        );
        assert!(read.await.unwrap().is_err());
    }
}
