//! Single-result service calls used by workflow preflight and connection health.
use super::{begin, error, without_secret};
use crate::model::Result;
use serde::{Serialize, de::DeserializeOwned};
use serde_json::Value;
use std::future::Future;

pub(crate) async fn protocol<T: Serialize + DeserializeOwned>(
    semantic: Value,
    secret: &str,
    live: impl Future<Output = Result<T>>,
) -> Result<T> {
    let Some(call) = begin("protocol", semantic)? else {
        return live.await;
    };
    if let Some(frames) = call.recorded() {
        if call.cancelled().is_some() {
            if !frames.is_empty() {
                return Err(error());
            }
            return call.require_cancellation(&mut 0).await;
        }
        if frames.len() != 1 {
            return Err(error());
        }
        call.wait(&mut 0, frames[0].0).await?;
        let result: Result<T> = serde_json::from_value(frames[0].1.clone()).map_err(|_| error())?;
        call.finish()?;
        return result;
    }
    let result = live.await;
    call.push(without_secret(serde_json::to_value(&result)?, secret))?;
    call.finish()?;
    result
}
