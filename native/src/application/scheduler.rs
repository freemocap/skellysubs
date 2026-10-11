use super::*;

pub(super) async fn scheduler(state: Arc<Application>, _app: tauri::AppHandle) {
    let client = match provider::client() {
        Ok(client) => client,
        Err(error) => {
            state.stop(error);
            return;
        }
    };
    let mut awakened = false;
    loop {
        let changed = state.scheduler_wake.notified();
        tokio::pin!(changed);
        changed.as_mut().enable();
        if let Err(error) = schedule_pending(&state, &client).await {
            state.stop(error);
            return;
        }
        if awakened {
            state.conversation_changed.notify_waiters();
        }
        awakened = tokio::select! {
            _ = changed => true,
            _ = tokio::time::sleep(Duration::from_millis(100)) => false,
        };
    }
}
pub(super) async fn schedule_pending(
    state: &Arc<Application>,
    client: &reqwest::Client,
) -> Result<()> {
    super::graph_execution::schedule(state, client).await
}
