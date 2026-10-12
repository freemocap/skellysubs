use super::{model::fault, *};
use serde::{Deserialize, Serialize};
use std::{collections::BTreeMap, sync::Arc};
use ts_rs::TS;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
pub enum AttemptState<F = Fault> {
    Prepared,
    Running,
    Available,
    Adopted,
    Failed(F),
    Unknown,
    Cancelled,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, TS)]
#[serde(deny_unknown_fields)]
pub struct Attempt<F = Fault, I = AttemptId, E = ExecutionId> {
    pub id: I,
    pub execution: E,
    pub state: AttemptState<F>,
    pub acquisition: Acquisition,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Work {
    pub execution: ExecutionId,
    pub artifact: String,
    pub operation: Contract,
    pub inputs: Values,
    pub resource: Resource,
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Capacity {
    pub local: usize,
    pub provider: usize,
}

/// Commands/results are explicit state-machine inputs. The journal contains
/// source data; it is a checkpoint format, NEVER a diagnostic/inspection payload.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub enum Event {
    Begin {
        run: String,
        artifact: String,
        inputs: Values,
        scope: String,
        policy: BTreeMap<String, Activation>,
    },
    Demand {
        run: String,
        node: String,
    },
    Pause {
        run: String,
        paused: bool,
    },
    Step {
        run: String,
    },
    Cancel {
        run: String,
        node: Option<String>,
    },
    Retry {
        run: String,
        node: String,
    },
    Advance(Capacity),
    Dispatch {
        execution: ExecutionId,
    },
    Settle {
        execution: ExecutionId,
        outcome: Result<Values>,
    },
    Observe(EvidenceSnapshot),
    SettleObserved(InvocationReport),
    Adopt {
        run: String,
        node: String,
        attempt: AttemptId,
    },
    Recover,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
pub enum Disposition {
    Disabled,
    Unrequested,
    Skipped,
    Waiting,
    Blocked,
    Ready,
    Paused,
    Held,
    Prepared,
    Running,
    Available,
    Adopted,
    Failed,
    Unknown,
    Cancelled,
}
/// Borrows the artifact used by execution. No topology copy or operation catalog.
/// Internal inspection only: constants and adapter faults can contain source
/// data. An explicit sensitivity policy is required before diagnostic export.
#[derive(Serialize)]
pub struct Inspection<'a, A = BTreeMap<String, Vec<&'a Attempt>>> {
    pub reasons: BTreeMap<String, Vec<Reason>>,
    pub artifact_id: &'a str,
    pub artifact: &'a Artifact,
    pub revision: u64,
    pub nodes: BTreeMap<String, Disposition>,
    pub activation: &'a BTreeMap<String, Activation>,
    pub attempts: A,
    pub stepping: Option<String>,
    pub step_available: bool,
}

#[derive(Clone, Default)]
pub struct Engine {
    pub(super) graphs: BTreeMap<String, Arc<super::plan::Plan>>,
    executables: BTreeMap<String, Arc<Executable>>,
    pub(super) state_limits: Option<StateLimits>,
    pub(super) state: super::state::RuntimeState,
    pub(super) journal_start: u64,
    journal: Vec<Event>,
}
/// A single-use invocation claimed from the state machine. Dropping it after
/// claim leaves an uncertain execution until settlement/recovery; never replay it.
pub struct Invocation {
    graph: Arc<Executable>,
    work: Work,
    pub(super) engine: Option<String>,
}
impl Invocation {
    pub async fn execute(self, limits: EvidenceLimits) -> InvocationReport {
        self.execute_with_provisional(limits, ProvisionalLimits { bytes: 0 })
            .await
    }
    pub async fn execute_with_provisional(
        self,
        limits: EvidenceLimits,
        provisional: ProvisionalLimits,
    ) -> InvocationReport {
        let context = InvocationContext::new(&self.work, self.engine, limits, provisional);
        let _guard = context.guard();
        let outcome = self.graph.execute(&self.work, context.clone()).await;
        context.finish(outcome)
    }
}
impl Engine {
    pub fn claim(&mut self, execution: ExecutionId) -> Result<Invocation> {
        let work = self
            .state
            .executions
            .get(&execution)
            .ok_or_else(|| fault(CoreFaultCode::UnknownExecution, "execution"))?
            .work
            .clone();
        let invocation = self.bind_invocation(work)?;
        self.apply(Event::Dispatch { execution })?;
        Ok(invocation)
    }
    pub(super) fn bind_invocation(&self, work: Work) -> Result<Invocation> {
        let graph = self
            .executables
            .get(&work.artifact)
            .ok_or_else(|| fault(CoreFaultCode::ExecutableRequired, "artifact"))?
            .clone();
        Ok(Invocation {
            graph,
            work,
            engine: None,
        })
    }
    /// Read the exact validated value before atomically adopting it with domain
    /// publication. No raw completion is reconstructed or parsed a second time.
    pub fn available(&self, run: &str, node: &str, attempt: AttemptId) -> Result<Values> {
        Ok(self
            .adoption(&mut &self.state, run, node, attempt)?
            .values()
            .clone())
    }
    pub fn new(graphs: impl IntoIterator<Item = Arc<Executable>>) -> Result<Self> {
        let mut engine = Self::default();
        for graph in graphs {
            if engine
                .graphs
                .insert(graph.identity().into(), graph.plan.clone())
                .is_some()
            {
                return Err(fault(CoreFaultCode::DuplicateArtifact, "graphs"));
            }
            engine.executables.insert(graph.identity().into(), graph);
        }
        Ok(engine)
    }
    pub(super) fn recorded(
        artifacts: &BTreeMap<String, Artifact>,
        limits: StateLimits,
    ) -> Result<Self> {
        let mut engine = Self::default().with_state_limits(limits)?;
        for (id, artifact) in artifacts {
            let plan = super::plan::Plan::new(artifact.clone())?;
            if plan.identity != *id {
                return Err(fault(
                    CoreFaultCode::CheckpointArtifactMismatch,
                    "artifacts",
                ));
            }
            engine.graphs.insert(id.clone(), Arc::new(plan));
        }
        Ok(engine)
    }
    /// Rejected events leave the machine unchanged, including IDs and history.
    /// This in-memory transaction must be committed with owner adoption when
    /// integrated with persistence; it is not a replacement for SQL transactions.
    pub fn apply(&mut self, event: Event) -> Result<Vec<Work>> {
        let (next, work) = self.candidate(event, &mut &self.state)?;
        *self = next;
        Ok(work)
    }
    /// Build one atomic event candidate using typed reads. Advance and cancellation
    /// also read records staged earlier within this event.
    pub(super) fn candidate(
        &self,
        event: Event,
        records: &mut impl super::record_access::RecordAccess,
    ) -> Result<(Self, Vec<Work>)> {
        self.revision()
            .checked_add(1)
            .ok_or_else(|| fault(CoreFaultCode::RevisionLimit, "event"))?;
        let mut next = self.clone();
        let work = next.transition(&event, &self.state, records)?;
        next.admit_records(0, 0, 0)?;
        next.journal.push(event);
        Ok((next, work))
    }
    /// Resident suffix only. Retired prefixes belong to the durable archive;
    /// its length must not be used as the engine's logical revision.
    pub fn journal(&self) -> &[Event] {
        &self.journal
    }
    /// Absolute logical revision; compaction never resets it.
    pub fn revision(&self) -> u64 {
        self.journal_start
            .checked_add(self.journal.len() as u64)
            .expect("validated journal revision")
    }
    /// Only the durable archive transition may retire this suffix.
    pub(super) fn rebase(&mut self) {
        self.journal_start = self.revision();
        self.journal.clear();
    }
    /// Replay validates artifact identity and all transitions without invoking
    /// handlers. Recovery makes uncertain executions explicit and pauses runs.
    pub fn restore(
        graphs: impl IntoIterator<Item = Arc<Executable>>,
        journal: &[Event],
    ) -> Result<Self> {
        let mut engine = Self::new(graphs)?;
        for event in journal {
            engine.apply(event.clone())?;
        }
        engine.apply(Event::Recover)?;
        Ok(engine)
    }
    pub fn inspect(&self, run: &str) -> Result<Inspection<'_>> {
        self.state.require_resident()?;
        self.run(run)?;
        self.inspect_with_attempts(run, self.state.attempt_history(run))
    }
    fn inspect_with_attempts<A>(&self, run: &str, attempts: A) -> Result<Inspection<'_, A>> {
        let r = self.run(run)?;
        self.inspect_with_attempts_using(&mut &self.state, run, r, attempts)
    }
    pub(super) fn inspect_with_attempts_using<'a, A>(
        &'a self,
        records: &mut impl super::record_access::RecordAccess,
        run: &str,
        r: &'a Run,
        attempts: A,
    ) -> Result<Inspection<'a, A>> {
        let graph = &self.graphs[&r.artifact];
        let states = self.states_using(records, r)?;
        let mut reasons = self.reasons(records, r, &states)?;
        for (node, state) in &states {
            if *state == Disposition::Ready && self.state.held.contains(&(run.into(), node.clone()))
            {
                reasons
                    .get_mut(node)
                    .unwrap()
                    .push(Reason::Admission(graph.operation(node).resource));
            }
        }
        let nodes = graph
            .artifact
            .definition
            .nodes
            .keys()
            .map(|n| {
                let state = states[n].clone();
                (
                    n.clone(),
                    if state == Disposition::Ready
                        && self.state.held.contains(&(run.into(), n.clone()))
                    {
                        Disposition::Held
                    } else {
                        state
                    },
                )
            })
            .collect();
        Ok(Inspection {
            reasons,
            artifact_id: &graph.identity,
            artifact: &graph.artifact,
            revision: self.revision(),
            nodes,
            activation: &r.policy,
            attempts,
            stepping: r.stepping.clone(),
            step_available: self.step_candidate(records, r)?.is_some(),
        })
    }
    pub fn outputs(&self, run: &str) -> Result<Option<Values>> {
        self.outputs_using(&mut &self.state, run)
    }
    pub(super) fn outputs_using(
        &self,
        records: &mut impl super::record_access::RecordAccess,
        run: &str,
    ) -> Result<Option<Values>> {
        let r = records.run(run)?;
        let graph = &self.graphs[&r.artifact];
        let states = self.states_using(records, &r)?;
        let mut values = Values::new();
        for (port, source) in &graph.artifact.definition.results {
            match self.resolve_using(records, &r, source, &states)? {
                super::dependencies::Resolved::Value(value) => {
                    values.insert(port.clone(), value);
                }
                super::dependencies::Resolved::Absent
                    if graph.artifact.definition.outputs[port].optional => {}
                _ => return Ok(None),
            }
        }
        Ok(Some(values))
    }
    pub(super) fn run(&self, id: &str) -> Result<&Run> {
        self.state
            .runs
            .get(id)
            .ok_or_else(|| fault(CoreFaultCode::UnknownRun, "run"))
    }
    pub(super) fn node(&self, run: &str, node: &str) -> Result<&Node> {
        let r = self.run(run)?;
        self.graphs[&r.artifact]
            .artifact
            .definition
            .nodes
            .get(node)
            .ok_or_else(|| fault(CoreFaultCode::UnknownNode, "node"))
    }
}
