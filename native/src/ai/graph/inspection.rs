use super::{dependencies::Resolved, *};
use std::collections::BTreeMap;

impl Engine {
    pub(super) fn reasons(
        &self,
        records: &mut impl super::record_access::RecordAccess,
        run: &Run,
        states: &BTreeMap<String, Disposition>,
    ) -> Result<BTreeMap<String, Vec<Reason>>> {
        let graph = &self.graphs[&run.artifact];
        graph
            .artifact
            .definition
            .nodes
            .iter()
            .map(|(id, node)| {
                let mut reasons = Vec::new();
                match states[id] {
                    Disposition::Disabled => reasons.push(Reason::PolicyDisabled),
                    Disposition::Unrequested => reasons.push(Reason::DemandRequired),
                    Disposition::Cancelled => reasons.push(Reason::Revoked),
                    Disposition::Paused => reasons.push(Reason::Paused),
                    Disposition::Skipped => reasons.push(Reason::GuardFalse),
                    Disposition::Waiting | Disposition::Blocked => {
                        if let Some(guard) = &node.guard
                            && !matches!(
                                self.resolve_presence_using(records, run, guard, states)?,
                                Resolved::Value(_)
                            )
                        {
                            reasons.push(Reason::GuardUnavailable);
                        }
                        for parent in &node.after {
                            if states[parent] != Disposition::Adopted {
                                reasons.push(Reason::Control {
                                    parent: parent.clone(),
                                    state: states[parent].clone(),
                                });
                            }
                        }
                        for (port, source) in &node.inputs {
                            let availability = match self
                                .resolve_presence_using(records, run, source, states)?
                            {
                                Resolved::Value(_) => None,
                                Resolved::Absent if graph.operation(id).inputs[port].optional => {
                                    None
                                }
                                Resolved::Absent => Some(Availability::Absent),
                                Resolved::Waiting => Some(Availability::Waiting),
                                Resolved::Blocked => Some(Availability::Blocked),
                            };
                            if let Some(availability) = availability {
                                reasons.push(Reason::Input {
                                    port: port.clone(),
                                    producer: match source {
                                        Source::Output { node, .. } => Some(node.clone()),
                                        _ => None,
                                    },
                                    availability,
                                });
                            }
                        }
                    }
                    _ => (),
                }
                if let Some(attempt) = run.current.get(id) {
                    let row = records.attempt(*attempt)?;
                    let a = &row.attempt;
                    reasons.push(Reason::Attempt {
                        id: a.id,
                        state: a.state.clone(),
                    });
                }
                Ok((id.clone(), reasons))
            })
            .collect()
    }
}
