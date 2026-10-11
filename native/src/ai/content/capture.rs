//! Invocation-scoped transport tee. No headers, URLs or diagnostics are captured.
use super::{Result, Write};
use sha2::{Digest, Sha256};
use std::{
    future::Future,
    sync::{Arc, Mutex},
};

pub type Writer = Arc<dyn Fn(Write) -> Result<()> + Send + Sync>;
tokio::task_local! { static ACTIVE: Arc<Capture>; }
struct Stream {
    id: String,
    hash: Sha256,
    pending: Vec<u8>,
    secret: zeroize::Zeroizing<Vec<u8>>,
    removed: bool,
}
struct Capture {
    writer: Writer,
    stream: Mutex<Option<Stream>>,
}
impl Capture {
    fn finish(&self, complete: bool) -> Result<()> {
        let Some(mut stream) = self.stream.lock().map_err(|_| super::invalid())?.take() else {
            return Ok(());
        };
        stream.flush(&self.writer, true)?;
        emit(
            &self.writer,
            Write::Finish {
                id: stream.id,
                complete,
                digest: format!("{:x}", stream.hash.finalize()),
                credentials_removed: stream.removed,
            },
        )
    }
}
impl Drop for Capture {
    fn drop(&mut self) {
        // The application writer promotes persistence errors to a visible fatal
        // storage error. An interrupted stream is never marked complete.
        let _ = self.finish(false);
    }
}
impl Stream {
    fn flush(&mut self, writer: &Writer, final_chunk: bool) -> Result<()> {
        let mut bytes = Vec::new();
        let mut offset = 0;
        while offset < self.pending.len() {
            if !final_chunk
                && !self.secret.is_empty()
                && self.pending.len() - offset < self.secret.len()
            {
                break;
            }
            if !self.secret.is_empty() && self.pending[offset..].starts_with(&self.secret) {
                bytes.extend_from_slice(b"<credential removed>");
                offset += self.secret.len();
                self.removed = true;
            } else {
                bytes.push(self.pending[offset]);
                offset += 1;
            }
        }
        self.pending.drain(..offset);
        self.hash.update(&bytes);
        if !bytes.is_empty() {
            emit(
                writer,
                Write::Append {
                    id: self.id.clone(),
                    bytes,
                },
            )?;
        }
        Ok(())
    }
}
pub async fn scope<T>(writer: Writer, future: impl Future<Output = T>) -> T {
    ACTIVE
        .scope(
            Arc::new(Capture {
                writer,
                stream: Mutex::new(None),
            }),
            future,
        )
        .await
}

pub fn request(kind: &str, media_type: &str, bytes: &[u8], key: &str) -> Result<()> {
    ACTIVE
        .try_with(|capture| {
            capture.finish(false)?;
            let id = uuid::Uuid::new_v4().to_string();
            emit(
                &capture.writer,
                Write::Begin {
                    id: id.clone(),
                    kind: format!("{kind}.request"),
                    media_type: media_type.into(),
                },
            )?;
            let mut request = Stream {
                id,
                hash: Sha256::new(),
                pending: bytes.to_vec(),
                secret: zeroize::Zeroizing::new(key.as_bytes().to_vec()),
                removed: false,
            };
            request.flush(&capture.writer, true)?;
            emit(
                &capture.writer,
                Write::Finish {
                    id: request.id,
                    complete: true,
                    digest: format!("{:x}", request.hash.finalize()),
                    credentials_removed: request.removed,
                },
            )?;
            let id = uuid::Uuid::new_v4().to_string();
            emit(
                &capture.writer,
                Write::Begin {
                    id: id.clone(),
                    kind: format!("{kind}.response"),
                    media_type: "application/octet-stream".into(),
                },
            )?;
            *capture.stream.lock().map_err(|_| super::invalid())? = Some(Stream {
                id,
                hash: Sha256::new(),
                pending: Vec::new(),
                secret: zeroize::Zeroizing::new(key.as_bytes().to_vec()),
                removed: false,
            });
            Ok(())
        })
        .unwrap_or(Ok(()))
}
pub fn chunk(bytes: &[u8]) -> Result<()> {
    ACTIVE
        .try_with(|capture| {
            let mut guard = capture.stream.lock().map_err(|_| super::invalid())?;
            if let Some(stream) = guard.as_mut() {
                stream.pending.extend_from_slice(bytes);
                stream.flush(&capture.writer, false)?;
            }
            Ok(())
        })
        .unwrap_or(Ok(()))
}
pub fn complete() -> Result<()> {
    ACTIVE
        .try_with(|capture| capture.finish(true))
        .unwrap_or(Ok(()))
}
pub fn active() -> bool {
    ACTIVE.try_with(|_| ()).is_ok()
}

/// Read a rejected body when the existing error path would otherwise discard it.
pub async fn rejected(mut response: reqwest::Response) -> Result<()> {
    if !active() {
        return Ok(());
    }
    let mut total = 0;
    while let Some(bytes) = response
        .chunk()
        .await
        .map_err(|e| crate::diagnostics::response::network(&e, "original_response"))?
    {
        total += bytes.len();
        if total > 8 * 1024 * 1024 {
            return Err(super::invalid());
        }
        chunk(&bytes)?;
    }
    complete()
}

/// Standalone source bytes, associated with the same native execution.
pub fn source(kind: &str, media_type: &str, bytes: &[u8], key: &str) -> Result<()> {
    ACTIVE
        .try_with(|capture| {
            let id = uuid::Uuid::new_v4().to_string();
            emit(
                &capture.writer,
                Write::Begin {
                    id: id.clone(),
                    kind: kind.into(),
                    media_type: media_type.into(),
                },
            )?;
            let mut stream = Stream {
                id,
                hash: Sha256::new(),
                pending: bytes.to_vec(),
                secret: zeroize::Zeroizing::new(key.as_bytes().to_vec()),
                removed: false,
            };
            stream.flush(&capture.writer, true)?;
            emit(
                &capture.writer,
                Write::Finish {
                    id: stream.id,
                    complete: true,
                    digest: format!("{:x}", stream.hash.finalize()),
                    credentials_removed: stream.removed,
                },
            )
        })
        .unwrap_or(Ok(()))
}

#[cfg(feature = "workflow-capture")]
tokio::task_local! { static TAP: Writer; }
fn emit(writer: &Writer, event: Write) -> Result<()> {
    #[cfg(feature = "workflow-capture")]
    let recorded = event.clone();
    // A development tape failure must not discard already purchased content.
    writer(event)?;
    #[cfg(feature = "workflow-capture")]
    TAP.try_with(|tap| tap(recorded)).unwrap_or(Ok(()))?;
    Ok(())
}

#[cfg(feature = "workflow-capture")]
pub async fn tap<T>(writer: Writer, future: impl Future<Output = T>) -> T {
    struct Finish;
    impl Drop for Finish {
        fn drop(&mut self) {
            let _ = ACTIVE.try_with(|capture| capture.finish(false));
        }
    }
    TAP.scope(writer, async {
        let _finish = Finish;
        future.await
    })
    .await
}
#[cfg(feature = "workflow-capture")]
pub fn restore(event: Write) -> Result<()> {
    ACTIVE
        .try_with(|capture| (capture.writer)(event))
        .unwrap_or(Ok(()))
}
