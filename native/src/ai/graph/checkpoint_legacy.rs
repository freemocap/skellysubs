//! Frozen physical format-2 records. These have no execution semantics: they
//! decode retained evidence and compare it with current native replay only.
use super::{
    state::{Execution, RuntimeState},
    *,
};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct InlineRun {
    artifact: String,
    inputs: Values,
    scope: String,
    policy: BTreeMap<String, Activation>,
    demanded: BTreeSet<String>,
    retry_requested: BTreeSet<String>,
    cancelled: BTreeSet<String>,
    attempts: BTreeMap<String, Vec<Attempt>>,
    paused: bool,
    active: bool,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct InlineState {
    runs: BTreeMap<String, InlineRun>,
    executions: BTreeMap<ExecutionId, Execution>,
    next_id: u64,
    capacity: Capacity,
    held: BTreeSet<(String, String)>,
}

impl InlineState {
    pub fn has_structured_evidence(&self) -> bool {
        self.executions.values().any(|e| {
            e.evidence
                .as_ref()
                .is_some_and(ExecutionEvidence::has_structured)
        })
    }
    pub fn has_provisional(&self) -> bool {
        self.executions
            .values()
            .any(|e| e.evidence.as_ref().is_some_and(|e| e.provisional.is_some()))
    }
    pub fn has_evidence(&self) -> bool {
        self.executions.values().any(|e| e.evidence.is_some())
    }
    pub fn matches(&self, state: &RuntimeState) -> bool {
        self == &Self::capture(state)
    }

    pub fn capture(state: &RuntimeState) -> Self {
        let runs = state
            .runs
            .iter()
            .map(|(id, run)| {
                let attempts = state
                    .attempt_history(id)
                    .into_iter()
                    .map(|(node, attempts)| (node, attempts.into_iter().cloned().collect()))
                    .collect();
                (
                    id.clone(),
                    InlineRun {
                        artifact: run.artifact.clone(),
                        inputs: run.inputs.clone(),
                        scope: run.scope.clone(),
                        policy: run.policy.clone(),
                        demanded: run.demanded.clone(),
                        retry_requested: run.retry_requested.clone(),
                        cancelled: run.cancelled.clone(),
                        attempts,
                        paused: run.paused,
                        active: run.active,
                    },
                )
            })
            .collect();
        Self {
            runs,
            executions: state
                .executions
                .iter()
                .map(|(id, execution)| (*id, execution.clone()))
                .collect(),
            next_id: state.next_id,
            capacity: state.capacity,
            held: state.held.clone(),
        }
    }
}
