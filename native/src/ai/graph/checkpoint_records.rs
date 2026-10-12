use super::{record_evidence::RecordEvidence, state::RuntimeState, *};
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

/// Format 4 retains primary-record commitments and scalar machine state, never
/// record payloads. Replay verifies these commitments against the complete
/// archived native event history before any current stored row is trusted.
#[derive(Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct RecordState {
    rows: Vec<(RecordKey, [u8; 32])>,
    next_id: u64,
    capacity: Capacity,
    held: BTreeSet<(String, String)>,
}

impl RecordState {
    pub fn capture(state: &RuntimeState, evidence: &RecordEvidence) -> Self {
        Self {
            rows: evidence.commitments(),
            next_id: state.next_id,
            capacity: state.capacity,
            held: state.held.clone(),
        }
    }

    pub fn matches(&self, state: &RuntimeState) -> Result<bool> {
        // Replay input is already byte-bounded by checkpoint/history admission.
        // The synthetic stamp is only a local digest-catalog construction seed.
        state.require_resident()?;
        let evidence = RecordEvidence::from_state(
            state,
            &Stamp {
                engine: String::new(),
                revision: 0,
                checksum: String::new(),
            },
            usize::MAX,
        )?;
        Ok(self == &Self::capture(state, &evidence))
    }
}
