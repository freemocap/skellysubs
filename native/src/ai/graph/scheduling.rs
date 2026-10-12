use super::{
    compile::digest,
    dependencies::Resolved,
    model::fault,
    record_access::RecordAccess,
    staged_access::StagedAccess,
    state::{AttemptRecord, Execution, RuntimeState},
    *,
};

impl Engine {
    fn schedule_node(
        &mut self,
        run: &str,
        node: &str,
        capacity: Capacity,
        base: &RuntimeState,
        records: &mut impl RecordAccess,
    ) -> Result<()> {
        let mut records = StagedAccess {
            base,
            candidate: &self.state,
            records,
        };
        let r = records.run(run)?;
        let states = self.states_using(&mut records, &r)?;
        if states[node] != Disposition::Ready {
            return Ok(());
        }
        let fresh = r.retry_requested.contains(node);
        let graph = &self.graphs[&r.artifact];
        let n = &graph.artifact.definition.nodes[node];
        let mut inputs = Values::new();
        for (name, source) in &n.inputs {
            match self.resolve_using(&mut records, &r, source, &states)? {
                Resolved::Value(v) => {
                    inputs.insert(name.clone(), v);
                }
                Resolved::Absent if graph.operation(node).inputs[name].optional => (),
                _ => return Err(fault(CoreFaultCode::InputsUnavailable, "node")),
            }
        }
        let op = graph.operation(node);
        // Include actual type definitions and implementation identity. Product
        // ownership stays outside equality; authority/configuration scope does not.
        let contracts: Vec<_> = op
            .inputs
            .values()
            .chain(op.outputs.values())
            .map(|p| (&p.contract, &graph.artifact.types[&p.contract]))
            .collect();
        let key = digest(&(op, contracts, &inputs, &r.scope))?;
        let mut shared = None;
        if !fresh && op.reuse == Reuse::Exact {
            for id in self.state.executions.key_ids(&key) {
                let e = records.execution(id)?;
                if !e.unknown && !matches!(e.outcome, Some(Err(_))) {
                    shared = Some((
                        id,
                        if e.outcome.is_some() {
                            AttemptState::Available
                        } else if e.dispatched {
                            AttemptState::Running
                        } else {
                            AttemptState::Prepared
                        },
                    ));
                    break;
                }
            }
        }
        // Preserve the existing allocator-overflow rejection even for held work,
        // without assigning IDs or constructing speculative records.
        self.state
            .next_id
            .checked_add(1 + u64::from(shared.is_none()))
            .ok_or_else(|| fault(CoreFaultCode::IdentityLimit, "engine"))?;
        // A resource hold creates no records. Decide it before record admission,
        // while a pending join or retained result needs no additional resource slot.
        if shared.is_none() {
            let limit = match op.resource {
                Resource::Local => capacity.local,
                Resource::Provider => capacity.provider,
            };
            let mut busy = 0;
            for id in self.state.executions.unresolved_ids() {
                let e = records.execution(id)?;
                if e.work.resource == op.resource
                    && (e.dispatched || self.has_consumer_using(&mut records, id, false)?)
                {
                    busy += 1;
                }
            }
            if busy >= limit {
                self.state.held.insert((run.into(), node.into()));
                return Ok(());
            }
        }
        self.admit_records(0, 1, usize::from(shared.is_none()))?;
        let artifact = r.artifact.clone();
        let operation = op.contract.clone();
        let resource = op.resource;
        self.state.next_id = self
            .state
            .next_id
            .checked_add(1)
            .ok_or_else(|| fault(CoreFaultCode::IdentityLimit, "engine"))?;
        let attempt = AttemptId(self.state.next_id);
        let (execution, state, acquisition) = if let Some((id, state)) = shared {
            let mode = if state == AttemptState::Available {
                Acquisition::Retained
            } else {
                Acquisition::Subscribed
            };
            (id, state, mode)
        } else {
            self.state.next_id = self
                .state
                .next_id
                .checked_add(1)
                .ok_or_else(|| fault(CoreFaultCode::IdentityLimit, "engine"))?;
            let execution = ExecutionId(self.state.next_id);
            self.state.executions.insert(
                execution,
                Execution {
                    key,
                    work: Work {
                        execution,
                        artifact,
                        operation,
                        inputs,
                        resource,
                    },
                    outcome: None,
                    unknown: false,
                    dispatched: false,
                    evidence: None,
                },
            );
            (execution, AttemptState::Prepared, Acquisition::Produced)
        };
        self.state.attempts.insert(
            attempt,
            AttemptRecord {
                run: run.into(),
                node: node.into(),
                attempt: Attempt {
                    id: attempt,
                    execution,
                    state,
                    acquisition,
                },
            },
        );
        self.state.runs.load(run.into(), r.clone());
        self.state
            .runs
            .get_mut(run)
            .unwrap()
            .current
            .insert(node.into(), attempt);
        self.state
            .runs
            .get_mut(run)
            .unwrap()
            .retry_requested
            .remove(node);
        Ok(())
    }
    pub(super) fn advance(
        &mut self,
        capacity: Capacity,
        base: &RuntimeState,
        records: &mut impl RecordAccess,
    ) -> Result<Vec<Work>> {
        self.state.capacity = capacity;
        self.state.held.clear();
        let mut candidates = Vec::new();
        for id in self.state.runs.keys() {
            let owner = records.run(id)?;
            for node in self.graphs[&owner.artifact]
                .artifact
                .definition
                .nodes
                .keys()
            {
                candidates.push((id.clone(), node.clone()));
            }
        }
        for (run, node) in candidates {
            self.schedule_node(&run, &node, capacity, base, records)?;
        }
        // Include subscriptions and producers staged earlier in this same event.
        let mut records = StagedAccess {
            base,
            candidate: &self.state,
            records,
        };
        let mut work = Vec::new();
        for id in self.state.executions.unresolved_ids() {
            let producer = records.execution(id)?;
            if !producer.dispatched && self.has_consumer_using(&mut records, id, false)? {
                work.push(producer.work.clone());
            }
        }
        Ok(work)
    }
}
