//! Tee already-sanitized originals into the same timed transport tape. Replay
//! restores these captured bytes; it never invents wire bodies from decoded data.
use super::*;
impl Call {
    pub(super) async fn originals<T>(&self, future: impl std::future::Future<Output = T>) -> T {
        let id = self.id;
        let recording = self.recording && self.replay.is_none();
        let writer = std::sync::Arc::new(move |event: crate::ai::content::Write| {
            if !recording {
                return Ok(());
            }
            let payload = serde_json::json!({"original_content":event});
            locked()?.as_mut().ok_or_else(error)?.push(id, payload)
        });
        crate::ai::content::capture::tap(writer, future).await
    }
}
pub(super) fn restore(payload: &Value) -> Result<bool> {
    let Some(value) = payload.get("original_content") else {
        return Ok(false);
    };
    crate::ai::content::capture::restore(
        serde_json::from_value(value.clone()).map_err(|_| error())?,
    )?;
    Ok(true)
}
