use super::{
    encoding::bounded_json,
    model::fault,
    state::{AttemptRecord, Execution, RuntimeState},
    *,
};
use serde::{Deserialize, Serialize};

/// Native storage identity. This is private source data, not an inspection DTO.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum RecordKey {
    Run(String),
    Attempt(AttemptId),
    Execution(ExecutionId),
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Envelope<K, V> {
    pub format: u32,
    pub key: K,
    pub value: V,
}

#[derive(Serialize)]
#[serde(untagged)]
enum Value<'a> {
    Run(&'a Run),
    Attempt(&'a AttemptRecord),
    Execution(&'a Execution),
}

/// A borrowed primary record; no duplicated storage model or transition logic.
pub struct RecordWrite<'a> {
    key: RecordKey,
    value: Value<'a>,
}

impl RecordWrite<'_> {
    pub fn key(&self) -> &RecordKey {
        &self.key
    }

    /// Execution format 4 with structured metadata, 3 with provisional content,
    /// 2 with scalar metadata only;
    /// run format 2 carries a step permission; other records retain format 1.
    /// Only exact primary records are serialized; indexes
    /// rebuild on load. Adapters must never log this content-bearing envelope.
    pub fn bytes(&self, max_bytes: usize) -> Result<Vec<u8>> {
        bounded_json(
            &Envelope {
                format: if matches!(&self.value, Value::Run(r) if r.stepping.is_some()) {
                    2
                } else if matches!(&self.value, Value::Execution(e) if e.evidence.as_ref().is_some_and(ExecutionEvidence::has_structured))
                {
                    4
                } else if matches!(&self.value, Value::Execution(e) if e.evidence.as_ref().is_some_and(|e| e.provisional.is_some()))
                {
                    3
                } else if matches!(&self.value, Value::Execution(e) if e.evidence.is_some()) {
                    2
                } else {
                    1
                },
                key: &self.key,
                value: &self.value,
            },
            max_bytes,
            CoreFaultCode::RecordByteLimit,
        )
    }
}

/// Upserts selected from the native candidate, atomically committed with its
/// checkpoint and owner effects. There is no record deletion transition.
#[derive(Default)]
pub struct RecordChanges<'a>(Vec<RecordWrite<'a>>);

impl<'a> RecordChanges<'a> {
    pub fn iter(&self) -> impl ExactSizeIterator<Item = &RecordWrite<'a>> {
        self.0.iter()
    }

    pub(super) fn between(previous: Option<&RuntimeState>, next: &'a RuntimeState) -> Self {
        let mut writes = Vec::new();
        for (key, row) in next.runs.iter() {
            if previous
                .and_then(|s| s.runs.get(key))
                .is_none_or(|old| !std::ptr::eq(old, row) && old != row)
            {
                writes.push(RecordWrite {
                    key: RecordKey::Run(key.clone()),
                    value: Value::Run(row),
                });
            }
        }
        for (key, row) in next.attempts.iter() {
            if previous
                .and_then(|s| s.attempts.get(key))
                .is_none_or(|old| !std::ptr::eq(old, row) && old != row)
            {
                writes.push(RecordWrite {
                    key: RecordKey::Attempt(*key),
                    value: Value::Attempt(row),
                });
            }
        }
        for (key, row) in next.executions.iter() {
            if previous
                .and_then(|s| s.executions.get(key))
                .is_none_or(|old| !std::ptr::eq(old, row) && old != row)
            {
                writes.push(RecordWrite {
                    key: RecordKey::Execution(*key),
                    value: Value::Execution(row),
                });
            }
        }
        Self(writes)
    }
}

/// Reads of the current native record set, bound to the complete checkpoint
/// stamp. Each read/count must check that stamp in the same read transaction.
/// Missing, changed, oversized or unreadable records are errors, never absence.
pub trait RecordStore {
    fn record_count(&mut self, expected: &Stamp) -> Result<usize>;
    /// Enforce max_bytes before allocating the body, including concurrent changes.
    fn read_record(
        &mut self,
        expected: &Stamp,
        key: &RecordKey,
        max_bytes: usize,
    ) -> Result<Vec<u8>>;
}

pub(super) fn verify(
    state: &RuntimeState,
    stamp: &Stamp,
    max_bytes: usize,
    store: &mut impl RecordStore,
) -> Result<()> {
    state.require_resident()?;
    let rows = RecordChanges::between(None, state);
    if store.record_count(stamp)? != rows.iter().len() {
        return Err(fault(CoreFaultCode::RecordMismatch, "records"));
    }
    for row in rows.iter() {
        let expected = row.bytes(max_bytes)?;
        let actual = store.read_record(stamp, row.key(), max_bytes)?;
        if actual.len() > max_bytes {
            return Err(fault(CoreFaultCode::RecordByteLimit, "record"));
        }
        if actual != expected {
            return Err(fault(CoreFaultCode::RecordMismatch, "record"));
        }
    }
    Ok(())
}
