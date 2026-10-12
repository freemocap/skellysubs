use super::{records::Records, *};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Run {
    pub(super) artifact: String,
    pub(super) inputs: Values,
    /// Caller-supplied validated authority/configuration scope, opaque to the core.
    pub(super) scope: String,
    pub(super) policy: BTreeMap<String, Activation>,
    pub(super) demanded: BTreeSet<String>,
    pub(super) retry_requested: BTreeSet<String>,
    pub(super) cancelled: BTreeSet<String>,
    pub(super) current: BTreeMap<String, AttemptId>,
    pub(super) paused: bool,
    pub(super) active: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(super) stepping: Option<String>,
}

impl Run {
    pub(super) fn permits(&self, node: &str) -> bool {
        !self.paused || self.stepping.as_deref() == Some(node)
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct AttemptRecord {
    pub run: String,
    pub node: String,
    pub attempt: Attempt,
}

impl AttemptRecord {
    pub fn eligible_consumer(&self, owner: &Run, allow_paused: bool) -> bool {
        owner.active
            && (allow_paused || owner.permits(&self.node))
            && !owner.cancelled.contains(&self.node)
            && matches!(
                self.attempt.state,
                AttemptState::Prepared | AttemptState::Running
            )
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Execution {
    pub work: Work,
    pub key: String,
    pub outcome: Option<Result<Values>>,
    pub unknown: bool,
    pub dispatched: bool,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "present_evidence"
    )]
    pub evidence: Option<ExecutionEvidence>,
}

// Absence is the historical representation. An explicitly present null is not
// silently dropped during canonical re-encoding of an older physical format.
fn present_evidence<'de, D: serde::Deserializer<'de>>(
    decoder: D,
) -> std::result::Result<Option<ExecutionEvidence>, D::Error> {
    ExecutionEvidence::deserialize(decoder).map(Some)
}

/// Canonical native state used directly by the interpreter and persisted bases.
/// Artifacts, executable capabilities, admission policy and event history have
/// separate owners. Deserialization is evidence, not permission to install state;
/// checkpoint replay still validates each retained base against native execution.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct RuntimeState {
    pub runs: Records<String, Run>,
    pub attempts: super::attempts::Attempts,
    pub executions: super::executions::Executions,
    pub next_id: u64,
    pub capacity: Capacity,
    pub held: BTreeSet<(String, String)>,
}

impl RuntimeState {
    pub fn resident_usage(&self) -> StateUsage {
        StateUsage {
            runs: self.runs.resident_len(),
            attempts: self.attempts.resident_len(),
            executions: self.executions.resident_len(),
        }
    }

    pub fn evict(&mut self) {
        self.runs.evict();
        self.attempts.evict();
        self.executions.evict();
    }

    pub fn require_resident(&self) -> Result<()> {
        if self.runs.len() != self.runs.resident_len()
            || self.attempts.len() != self.attempts.resident_len()
            || self.executions.len() != self.executions.resident_len()
        {
            return Err(super::model::fault(
                CoreFaultCode::RecordNotResident,
                "records",
            ));
        }
        Ok(())
    }

    pub fn run_attempts(&self, run: &str) -> impl ExactSizeIterator<Item = &AttemptRecord> {
        self.attempts.for_run(run)
    }

    pub fn attempt_history<'a>(&'a self, run: &str) -> BTreeMap<String, Vec<&'a Attempt>> {
        let mut history = BTreeMap::<String, Vec<&Attempt>>::new();
        for record in self.run_attempts(run) {
            history
                .entry(record.node.clone())
                .or_default()
                .push(&record.attempt);
        }
        history
    }
}
