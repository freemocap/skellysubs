use super::{model::fault, state::RuntimeState, *};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;

/// Derived integrity evidence for one committed revision. Contains no record
/// payloads or scheduling policy; rebuilt from validated native replay at startup.
/// Private hashes are content-derived and must not enter diagnostic projections.
#[derive(Clone)]
pub(super) struct RecordEvidence {
    stamp: Stamp,
    rows: BTreeMap<RecordKey, [u8; 32]>,
}

impl RecordEvidence {
    pub fn commitments(&self) -> Vec<(RecordKey, [u8; 32])> {
        self.rows
            .iter()
            .map(|(key, digest)| (key.clone(), *digest))
            .collect()
    }
    pub fn from_state(state: &RuntimeState, stamp: &Stamp, max_bytes: usize) -> Result<Self> {
        state.require_resident()?;
        Self {
            stamp: stamp.clone(),
            rows: BTreeMap::new(),
        }
        .advance(stamp, &RecordChanges::between(None, state), max_bytes)
    }

    /// Prepared with the native write set; installed only after known commit
    /// success. Rejected/uncertain commits cannot publish this candidate catalog.
    pub fn advance(
        &self,
        stamp: &Stamp,
        changes: &RecordChanges<'_>,
        max_bytes: usize,
    ) -> Result<Self> {
        if self.stamp.engine != stamp.engine || stamp.revision < self.stamp.revision {
            return Err(fault(CoreFaultCode::RecordStamp, "records"));
        }
        let mut next = self.clone();
        next.stamp = stamp.clone();
        for row in changes.iter() {
            next.rows.insert(
                row.key().clone(),
                Sha256::digest(row.bytes(max_bytes)?).into(),
            );
        }
        Ok(next)
    }

    pub fn stamp(&self) -> &Stamp {
        &self.stamp
    }

    fn expected(&self, key: &RecordKey) -> Result<&[u8; 32]> {
        self.rows.get(key).ok_or_else(|| {
            let (code, path) = match key {
                RecordKey::Run(_) => (CoreFaultCode::UnknownRun, "run"),
                RecordKey::Attempt(_) => (CoreFaultCode::UnknownAttempt, "attempt"),
                RecordKey::Execution(_) => (CoreFaultCode::UnknownExecution, "execution"),
            };
            fault(code, path)
        })
    }

    pub fn require(&self, key: &RecordKey) -> Result<()> {
        self.expected(key).map(|_| ())
    }

    pub fn check(&self, key: &RecordKey, bytes: &[u8]) -> Result<()> {
        let expected = self.expected(key)?;
        let actual: [u8; 32] = Sha256::digest(bytes).into();
        if &actual != expected {
            return Err(fault(CoreFaultCode::RecordMismatch, "record"));
        }
        Ok(())
    }
}
