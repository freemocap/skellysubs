//! Display responses derived from a validated, committed workspace snapshot.
//! This cache never supplies execution inputs or replaces protected storage reads.
use super::super::workspace_version::Version;
use crate::model::ConversationSnapshot;
use std::{collections::VecDeque, sync::Arc};

pub(in crate::application) struct Prepared {
    pub(super) revision: i32,
    fields: Arc<str>,
}
impl Prepared {
    pub(super) fn new(snapshot: &ConversationSnapshot) -> crate::model::Result<Arc<Self>> {
        // Keep the observation revision separate: navigation changes it without
        // changing any conversation display field. Encode the remaining fields once.
        let mut fields = serde_json::to_value(snapshot)?;
        fields
            .as_object_mut()
            .expect("conversation is an object")
            .remove("revision");
        Ok(Arc::new(Self {
            revision: snapshot.revision,
            fields: serde_json::to_string(&fields)?.into(),
        }))
    }
    pub(in crate::application) fn response(&self) -> tauri::ipc::Response {
        // A String response is already-encoded JSON, not a JSON string value.
        tauri::ipc::Response::new(format!(
            "{{\"revision\":{},{}",
            self.revision,
            &self.fields[1..]
        ))
    }
}

struct Entry {
    conversation: String,
    before: Option<i32>,
    value: Arc<Prepared>,
}
impl Entry {
    fn bytes(&self) -> usize {
        self.conversation.len()
            + self.value.fields.len()
            + std::mem::size_of::<Self>()
            + std::mem::size_of::<Prepared>()
            + 2 * std::mem::size_of::<usize>()
    }
}

#[derive(Default)]
pub(in crate::application) struct Cache {
    version: Option<Version>,
    entries: VecDeque<Entry>,
    bytes: usize,
}
impl Cache {
    const MAX_BYTES: usize = 32 * 1024 * 1024;

    /// OpenConversation changes only recency, metadata revision and its receipt.
    /// The command coordinator calls this under the same store guard as commit.
    pub(super) fn navigation_committed(
        &mut self,
        before: &Version,
        after: &Version,
        revision: i32,
    ) {
        if before == after || self.version.as_ref() != Some(before) {
            return;
        }
        self.version = Some(after.clone());
        for entry in &mut self.entries {
            entry.value = Arc::new(Prepared {
                revision,
                fields: entry.value.fields.clone(),
            });
        }
    }

    fn observe(&mut self, version: &Version) {
        if self.version.as_ref() != Some(version) {
            self.entries.clear();
            self.bytes = 0;
            self.version = Some(version.clone());
        }
    }

    pub(super) fn get(
        &mut self,
        version: &Version,
        conversation: &str,
        before: Option<i32>,
    ) -> Option<Arc<Prepared>> {
        self.observe(version);
        let index = self
            .entries
            .iter()
            .position(|entry| entry.conversation == conversation && entry.before == before)?;
        let entry = self.entries.remove(index)?;
        let value = entry.value.clone();
        self.entries.push_back(entry);
        Some(value)
    }

    /// Caller holds the store guard and has checked this is still its version.
    pub(super) fn insert(
        &mut self,
        version: &Version,
        conversation: String,
        before: Option<i32>,
        value: Arc<Prepared>,
    ) {
        self.observe(version);
        if let Some(index) = self
            .entries
            .iter()
            .position(|entry| entry.conversation == conversation && entry.before == before)
        {
            self.bytes -= self.entries.remove(index).unwrap().bytes();
        }
        let entry = Entry {
            conversation,
            before,
            value,
        };
        let bytes = entry.bytes();
        if bytes > Self::MAX_BYTES {
            return;
        }
        while self.bytes + bytes > Self::MAX_BYTES {
            self.bytes -= self.entries.pop_front().unwrap().bytes();
        }
        self.bytes += bytes;
        self.entries.push_back(entry);
    }
}

#[cfg(test)]
#[path = "cache_tests.rs"]
mod tests;
