use super::{
    model::fault,
    record_store::Envelope,
    state::{AttemptRecord, Execution, RuntimeState},
    *,
};
use serde::de::DeserializeOwned;
use std::collections::BTreeMap;
use std::sync::Arc;

/// Reuse verified native records only within one immutable protected read.
/// The wrapped StoredAccess retains stamp/hash validation and its aggregate read
/// budget bounds all newly loaded rows. No entry survives this operation, and
/// this adapter must not span an effect decision or commit.
pub(super) struct ReadAccess<R> {
    inner: R,
    runs: BTreeMap<String, Arc<Run>>,
    attempts: BTreeMap<AttemptId, Arc<AttemptRecord>>,
    executions: BTreeMap<ExecutionId, Arc<Execution>>,
}

impl<R> ReadAccess<R> {
    pub fn new(inner: R) -> Self {
        Self {
            inner,
            runs: BTreeMap::new(),
            attempts: BTreeMap::new(),
            executions: BTreeMap::new(),
        }
    }
}

impl<R: RecordAccess> RecordAccess for ReadAccess<R> {
    fn run(&mut self, id: &str) -> Result<Arc<Run>> {
        if let Some(row) = self.runs.get(id) {
            return Ok(row.clone());
        }
        let row = self.inner.run(id)?;
        self.runs.insert(id.into(), row.clone());
        Ok(row)
    }
    fn attempt(&mut self, id: AttemptId) -> Result<Arc<AttemptRecord>> {
        if let Some(row) = self.attempts.get(&id) {
            return Ok(row.clone());
        }
        let row = self.inner.attempt(id)?;
        self.attempts.insert(id, row.clone());
        Ok(row)
    }
    fn execution(&mut self, id: ExecutionId) -> Result<Arc<Execution>> {
        if let Some(row) = self.executions.get(&id) {
            return Ok(row.clone());
        }
        let row = self.inner.execution(id)?;
        self.executions.insert(id, row.clone());
        Ok(row)
    }
}

/// Typed native reads. Ownership is scoped to the caller; no hidden cache,
/// missing-record fallback or alternative operation semantics live here.
pub(super) trait RecordAccess {
    fn run(&mut self, id: &str) -> Result<Arc<Run>>;
    fn attempt(&mut self, id: AttemptId) -> Result<Arc<AttemptRecord>>;
    fn execution(&mut self, id: ExecutionId) -> Result<Arc<Execution>>;
}

impl RecordAccess for &RuntimeState {
    fn run(&mut self, id: &str) -> Result<Arc<Run>> {
        self.runs.get_shared(id).ok_or_else(|| {
            fault(
                if self.runs.contains_key(id) {
                    CoreFaultCode::RecordNotResident
                } else {
                    CoreFaultCode::UnknownRun
                },
                "run",
            )
        })
    }
    fn attempt(&mut self, id: AttemptId) -> Result<Arc<AttemptRecord>> {
        self.attempts.get_shared(&id).ok_or_else(|| {
            fault(
                if self.attempts.contains_key(&id) {
                    CoreFaultCode::RecordNotResident
                } else {
                    CoreFaultCode::UnknownAttempt
                },
                "attempt",
            )
        })
    }
    fn execution(&mut self, id: ExecutionId) -> Result<Arc<Execution>> {
        self.executions.get_shared(&id).ok_or_else(|| {
            fault(
                if self.executions.contains_key(&id) {
                    CoreFaultCode::RecordNotResident
                } else {
                    CoreFaultCode::UnknownExecution
                },
                "execution",
            )
        })
    }
}

/// Capacity reservation and the second, native effect transition may reuse
/// trusted resident rows. Cold rows require verified reads; a failed read is
/// propagated. This is never used to bypass the initial stored effect decision.
pub(super) struct LoadedAccess<'a, R> {
    pub state: &'a RuntimeState,
    pub records: &'a mut R,
}

impl<R: RecordAccess> RecordAccess for LoadedAccess<'_, R> {
    fn run(&mut self, id: &str) -> Result<Arc<Run>> {
        match self.state.runs.get_shared(id) {
            Some(row) => Ok(row),
            None => self.records.run(id),
        }
    }
    fn attempt(&mut self, id: AttemptId) -> Result<Arc<AttemptRecord>> {
        match self.state.attempts.get_shared(&id) {
            Some(row) => Ok(row),
            None => self.records.attempt(id),
        }
    }
    fn execution(&mut self, id: ExecutionId) -> Result<Arc<Execution>> {
        match self.state.executions.get_shared(&id) {
            Some(row) => Ok(row),
            None => self.records.execution(id),
        }
    }
}

/// Stored reads need only committed integrity evidence, not resident payloads.
/// The decoded record remains the native type used by the reducer.
pub(super) struct StoredAccess<'a, S> {
    pub evidence: &'a super::record_evidence::RecordEvidence,
    pub max_bytes: usize,
    pub store: &'a mut S,
}

impl<'a, S: RecordStore> StoredAccess<'a, S> {
    /// The resident state and integrity evidence must belong to the same engine.
    /// Still read and verify storage first; only redundant decoding is avoided.
    pub fn with_resident(self, state: &'a RuntimeState) -> VerifiedRead<'a, S> {
        VerifiedRead {
            stored: self,
            state,
        }
    }
    fn read<T: DeserializeOwned>(&mut self, key: RecordKey) -> Result<Arc<T>> {
        self.read_resident(key, None)
    }
    fn read_resident<T: DeserializeOwned>(
        &mut self,
        key: RecordKey,
        resident: Option<Arc<T>>,
    ) -> Result<Arc<T>> {
        self.evidence.require(&key)?;
        let bytes = self
            .store
            .read_record(self.evidence.stamp(), &key, self.max_bytes)?;
        if bytes.len() > self.max_bytes {
            return Err(fault(CoreFaultCode::RecordByteLimit, "record"));
        }
        self.evidence.check(&key, &bytes)?;
        if let Some(row) = resident {
            return Ok(row);
        }
        let row: Envelope<RecordKey, T> = serde_json::from_slice(&bytes)
            .map_err(|_| fault(CoreFaultCode::RecordMismatch, "record"))?;
        if !(row.format == 1
            || (row.format == 2 && matches!(&key, RecordKey::Run(_)))
            || (matches!(row.format, 2..=4) && matches!(&key, RecordKey::Execution(_))))
            || row.key != key
        {
            return Err(fault(CoreFaultCode::RecordMismatch, "record"));
        }
        Ok(Arc::new(row.value))
    }
}

pub(super) struct VerifiedRead<'a, S> {
    stored: StoredAccess<'a, S>,
    state: &'a RuntimeState,
}
impl<S: RecordStore> RecordAccess for VerifiedRead<'_, S> {
    fn run(&mut self, id: &str) -> Result<Arc<Run>> {
        self.stored
            .read_resident(RecordKey::Run(id.into()), self.state.runs.get_shared(id))
    }
    fn attempt(&mut self, id: AttemptId) -> Result<Arc<AttemptRecord>> {
        self.stored
            .read_resident(RecordKey::Attempt(id), self.state.attempts.get_shared(&id))
    }
    fn execution(&mut self, id: ExecutionId) -> Result<Arc<Execution>> {
        self.stored.read_resident(
            RecordKey::Execution(id),
            self.state.executions.get_shared(&id),
        )
    }
}

impl<S: RecordStore> RecordAccess for StoredAccess<'_, S> {
    fn run(&mut self, id: &str) -> Result<Arc<Run>> {
        self.read(RecordKey::Run(id.into()))
    }
    fn attempt(&mut self, id: AttemptId) -> Result<Arc<AttemptRecord>> {
        self.read(RecordKey::Attempt(id))
    }
    fn execution(&mut self, id: ExecutionId) -> Result<Arc<Execution>> {
        self.read(RecordKey::Execution(id))
    }
}
