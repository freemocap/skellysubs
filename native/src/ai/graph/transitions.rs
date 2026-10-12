use super::{model::fault, record_access::RecordAccess, registry::valid_name, *};
use std::collections::{BTreeMap, BTreeSet};

impl Engine {
    pub(super) fn transition(
        &mut self,
        event: &Event,
        base: &super::state::RuntimeState,
        records: &mut impl super::record_access::RecordAccess,
    ) -> Result<Vec<Work>> {
        match event {
            Event::Begin {
                run,
                artifact,
                inputs,
                scope,
                policy,
            } => {
                valid_name(run)?;
                valid_name(scope)?;
                if self.state.runs.contains_key(run) {
                    return Err(fault(CoreFaultCode::DuplicateRun, "run"));
                }
                let graph = self
                    .graphs
                    .get(artifact)
                    .ok_or_else(|| fault(CoreFaultCode::UnknownArtifact, "artifact"))?;
                graph.values(&graph.artifact.definition.inputs, inputs)?;
                if policy
                    .keys()
                    .any(|key| !graph.artifact.definition.nodes.contains_key(key))
                {
                    return Err(fault(CoreFaultCode::UnknownPolicyNode, "policy"));
                }
                self.admit_records(1, 0, 0)?;
                let policy = graph
                    .artifact
                    .definition
                    .nodes
                    .iter()
                    .map(|(id, node)| {
                        (
                            id.clone(),
                            policy.get(id).copied().unwrap_or(node.activation),
                        )
                    })
                    .collect();
                self.state.runs.insert(
                    run.clone(),
                    Run {
                        artifact: artifact.clone(),
                        inputs: inputs.clone(),
                        scope: scope.clone(),
                        policy,
                        demanded: BTreeSet::new(),
                        retry_requested: BTreeSet::new(),
                        cancelled: BTreeSet::new(),
                        current: BTreeMap::new(),
                        paused: false,
                        active: true,
                        stepping: None,
                    },
                );
            }
            Event::Demand { run, node } => {
                let owner = self.read_node(records, run, node)?;
                if owner.policy[node] != Activation::OnDemand || !owner.active {
                    return Err(fault(CoreFaultCode::InvalidDemand, "node"));
                }
                self.state.runs.load(run.clone(), owner);
                self.state
                    .runs
                    .get_mut(run)
                    .unwrap()
                    .demanded
                    .insert(node.clone());
            }
            Event::Pause { run, paused } => {
                let owner = records.run(run)?;
                self.state.runs.load(run.clone(), owner);
                self.state.runs.get_mut(run).unwrap().paused = *paused;
                self.state.runs.get_mut(run).unwrap().stepping = None;
            }
            Event::Step { run } => self.step(records, run)?,
            Event::Cancel { run, node } => {
                let owner = records.run(run)?;
                if let Some(node) = node {
                    self.read_node(records, run, node)?;
                }
                self.state.runs.load(run.clone(), owner.clone());
                let r = self.state.runs.get_mut(run).unwrap();
                if node.is_none() || node.as_ref() == r.stepping.as_ref() {
                    r.stepping = None;
                }
                if let Some(node) = node {
                    r.cancelled.insert(node.clone());
                } else {
                    r.active = false;
                }
                let affected: Vec<_> = owner
                    .current
                    .iter()
                    .filter(|(id, _)| node.as_ref().is_none_or(|n| n == *id))
                    .map(|(_, id)| *id)
                    .collect();
                for id in affected {
                    let row = records.attempt(id)?;
                    let a = &row.attempt;
                    if matches!(
                        a.state,
                        AttemptState::Prepared | AttemptState::Running | AttemptState::Available
                    ) {
                        self.state.attempts.load(row);
                        self.state.attempts.set_state(&id, AttemptState::Cancelled);
                    }
                }
                let mut staged = super::staged_access::StagedAccess {
                    base,
                    candidate: &self.state,
                    records,
                };
                let mut abandoned = Vec::new();
                for id in self.state.executions.unresolved_ids() {
                    let producer = staged.execution(id)?;
                    if !producer.dispatched && !self.has_consumer_using(&mut staged, id, true)? {
                        abandoned.push(producer);
                    }
                }
                for producer in abandoned {
                    let id = producer.work.execution;
                    self.state.executions.load(producer);
                    self.state.executions.set_outcome(
                        &id,
                        Err(fault(CoreFaultCode::CancelledBeforeDispatch, "execution")),
                    );
                }
            }
            Event::Retry { run, node } => {
                let r = self.read_node(records, run, node)?;
                if !r.active || r.cancelled.contains(node) {
                    return Err(fault(CoreFaultCode::InvalidRetry, "node"));
                }
                let id = r
                    .current
                    .get(node)
                    .ok_or_else(|| fault(CoreFaultCode::InvalidRetry, "node"))?;
                let previous = records.attempt(*id)?;
                if !matches!(
                    previous.attempt.state,
                    AttemptState::Failed(_) | AttemptState::Unknown
                ) {
                    return Err(fault(CoreFaultCode::InvalidRetry, "node"));
                }
                let owner = r;
                // Fresh retry records a new attempt; previous outcome remains immutable.
                self.state.runs.load(run.clone(), owner);
                self.state
                    .runs
                    .get_mut(run)
                    .unwrap()
                    .retry_requested
                    .insert(node.clone());
            }
            Event::Advance(capacity) => return self.advance(*capacity, base, records),
            Event::Dispatch { execution } => {
                let dispatch = self.dispatch(records, *execution)?;
                let work = dispatch.producer.work.clone();
                self.state.executions.load(dispatch.producer);
                self.state.executions.set_dispatched(execution);
                for id in dispatch.consumers {
                    self.state.attempts.load(records.attempt(id)?);
                    self.state.attempts.set_state(&id, AttemptState::Running);
                }
                return Ok(vec![work]);
            }
            Event::Observe(snapshot) => {
                self.record_evidence(
                    records,
                    &snapshot.identity,
                    &snapshot.observations,
                    &snapshot.evidence_failure,
                    &snapshot.provisional,
                    false,
                )?;
            }
            Event::SettleObserved(report) => {
                self.record_evidence(
                    records,
                    &report.identity,
                    &report.observations,
                    &report.evidence_failure,
                    &report.provisional,
                    true,
                )?;
                let outcome =
                    report.outcome.clone().and_then(|values| {
                        match report.evidence_failure.as_ref().or_else(|| {
                            report.provisional.as_ref().and_then(|p| p.failure.as_ref())
                        }) {
                            Some(error) => Err(error.clone()),
                            None => Ok(values),
                        }
                    });
                self.settle(records, report.identity.execution, &outcome)?;
            }
            Event::Settle { execution, outcome } => {
                if records.execution(*execution)?.evidence.is_some() {
                    return Err(fault(CoreFaultCode::EvidenceRequired, "execution"));
                }
                self.settle(records, *execution, outcome)?;
            }
            Event::Adopt { run, node, attempt } => {
                let adoption = self.adoption(records, run, node, *attempt)?;
                if adoption.owner.stepping.as_deref() == Some(node) {
                    self.state.runs.load(run.clone(), adoption.owner);
                    self.state.runs.get_mut(run).unwrap().stepping = None;
                }
                self.state.attempts.load(records.attempt(*attempt)?);
                self.state
                    .attempts
                    .set_state(attempt, AttemptState::Adopted);
            }
            Event::Recover => {
                let unresolved: Vec<_> = self.state.executions.unresolved_ids().collect();
                for id in unresolved {
                    let producer = records.execution(id)?;
                    let dispatched = producer.dispatched;
                    self.state.executions.load(producer);
                    if dispatched {
                        self.state.executions.set_unknown(&id);
                    } else {
                        self.state.executions.set_outcome(
                            &id,
                            Err(fault(CoreFaultCode::InterruptedBeforeDispatch, "execution")),
                        );
                    }
                }
                let runs: Vec<_> = self.state.runs.keys().cloned().collect();
                for run_id in runs {
                    let owner = records.run(&run_id)?;
                    let attempts: Vec<_> = owner.current.values().copied().collect();
                    if !owner.paused || owner.stepping.is_some() {
                        self.state.runs.load(run_id.clone(), owner.clone());
                        self.state.runs.get_mut(&run_id).unwrap().paused = true;
                        self.state.runs.get_mut(&run_id).unwrap().stepping = None;
                    }
                    for id in attempts {
                        let row = records.attempt(id)?;
                        let next = match row.attempt.state {
                            AttemptState::Running => Some(AttemptState::Unknown),
                            AttemptState::Prepared => Some(AttemptState::Failed(fault(
                                CoreFaultCode::InterruptedBeforeDispatch,
                                "execution",
                            ))),
                            _ => None,
                        };
                        if let Some(next) = next {
                            self.state.attempts.load(row);
                            self.state.attempts.set_state(&id, next);
                        }
                    }
                }
            }
        }
        Ok(Vec::new())
    }

    fn settle(
        &mut self,
        records: &mut impl RecordAccess,
        execution: ExecutionId,
        outcome: &Result<Values>,
    ) -> Result<()> {
        let ex = records.execution(execution)?;
        if !ex.dispatched || ex.outcome.is_some() || ex.unknown {
            return Err(fault(CoreFaultCode::AlreadySettled, "execution"));
        }
        let graph = &self.graphs[&ex.work.artifact];
        let op = &graph.artifact.operations[&ex.work.operation];
        // Malformed success settles as a validation failure and releases
        // capacity. It can never reach adoption or downstream consumers.
        let validated = outcome.clone().and_then(|values| {
            graph.values(&op.outputs, &values)?;
            Ok(values)
        });
        self.state.executions.load(ex);
        self.state
            .executions
            .set_outcome(&execution, validated.clone());
        self.transition_consumers(
            records,
            execution,
            AttemptState::Running,
            match validated {
                Ok(_) => AttemptState::Available,
                Err(error) => AttemptState::Failed(error),
            },
        )?;
        Ok(())
    }

    /// Select current consumers before taking mutable records. Unrelated runs
    /// remain shared with the previous committed state and historical readers.
    fn transition_consumers(
        &mut self,
        records: &mut impl super::record_access::RecordAccess,
        execution: ExecutionId,
        from: AttemptState,
        to: AttemptState,
    ) -> Result<()> {
        let mut affected = Vec::new();
        self.visit_consumers(records, execution, |_, row| {
            if row.attempt.state == from {
                affected.push(std::sync::Arc::new(row.clone()));
            }
        })?;
        for row in affected {
            if matches!(to, AttemptState::Failed(_) | AttemptState::Unknown) {
                let owner = records.run(&row.run)?;
                if owner.stepping.as_deref() == Some(&row.node) {
                    self.state.runs.load(row.run.clone(), owner);
                    self.state.runs.get_mut(&row.run).unwrap().stepping = None;
                }
            }
            let id = row.attempt.id;
            self.state.attempts.load(row);
            self.state.attempts.set_state(&id, to.clone());
        }
        Ok(())
    }
}
