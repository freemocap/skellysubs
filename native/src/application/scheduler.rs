use super::workspace_version::Version;
use super::*;

/// A fixed point observed by the authoritative scheduler, not a second ready queue.
#[derive(Default)]
pub(super) struct Idle(Option<(Version, bool)>);

impl Idle {
    pub(super) fn matches(&self, version: &Version, provider: bool) -> bool {
        self.0
            .as_ref()
            .is_some_and(|(seen, capacity)| seen == version && *capacity == provider)
    }

    pub(super) fn observe(
        &mut self,
        before: Version,
        after: Version,
        provider: bool,
        claimed: bool,
    ) {
        // A scan can adopt/cancel/advance without returning a claim. Such writes
        // can unlock work already visited in that scan: this is not yet idle.
        self.0 = (!claimed && before == after).then_some((after, provider));
    }

    pub(super) fn navigation_committed(&mut self, before: &Version, after: &Version) {
        if let Some((seen, _)) = &mut self.0
            && seen == before
        {
            *seen = after.clone();
        }
    }
}

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

#[cfg(test)]
#[path = "tests/scheduler_idle.rs"]
mod tests;
