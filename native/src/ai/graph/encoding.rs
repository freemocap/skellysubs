use super::{CoreFaultCode, Result, model::fault};
use std::io::Write;
struct BoundedWriter {
    bytes: Vec<u8>,
    limit: usize,
    exhausted: bool,
}
impl Write for BoundedWriter {
    fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
        if bytes.len() > self.limit.saturating_sub(self.bytes.len()) {
            self.exhausted = true;
            return Err(std::io::Error::other("serialization limit"));
        }
        self.bytes.extend_from_slice(bytes);
        Ok(bytes.len())
    }
    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}
pub(super) fn bounded_json(
    value: &impl serde::Serialize,
    limit: usize,
    code: CoreFaultCode,
) -> Result<Vec<u8>> {
    let mut writer = BoundedWriter {
        bytes: Vec::new(),
        limit,
        exhausted: false,
    };
    serde_json::to_writer(&mut writer, value).map_err(|_| {
        fault(
            if writer.exhausted {
                code
            } else {
                CoreFaultCode::Serialization
            },
            "serialization",
        )
    })?;
    Ok(writer.bytes)
}
