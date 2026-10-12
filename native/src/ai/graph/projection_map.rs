//! Total, structural mappings over the executable types. No layout or scheduling.
use super::*;

impl<V> Source<V> {
    fn map_value<W>(&self, map: &mut impl FnMut(&V) -> W) -> Source<W> {
        match self {
            Self::Input(port) => Source::Input(port.clone()),
            Self::Output { node, port } => Source::Output {
                node: node.clone(),
                port: port.clone(),
            },
            Self::Constant { contract, value } => Source::Constant {
                contract: contract.clone(),
                value: map(value),
            },
            Self::Absent(contract) => Source::Absent(contract.clone()),
        }
    }
}
impl<V> Definition<V> {
    fn map_values<W>(&self, map: &mut impl FnMut(&V) -> W) -> Definition<W> {
        Definition {
            contract: self.contract.clone(),
            inputs: self.inputs.clone(),
            outputs: self.outputs.clone(),
            nodes: self
                .nodes
                .iter()
                .map(|(id, n)| {
                    (
                        id.clone(),
                        Node {
                            operation: n.operation.clone(),
                            after: n.after.clone(),
                            activation: n.activation,
                            inputs: n
                                .inputs
                                .iter()
                                .map(|(p, s)| (p.clone(), s.map_value(map)))
                                .collect(),
                            guard: n.guard.as_ref().map(|s| s.map_value(map)),
                        },
                    )
                })
                .collect(),
            results: self
                .results
                .iter()
                .map(|(p, s)| (p.clone(), s.map_value(map)))
                .collect(),
            compositions: self
                .compositions
                .iter()
                .map(|(id, b)| {
                    (
                        id.clone(),
                        Boundary {
                            artifact: b.artifact.clone(),
                            source: Box::new(b.source.map_values(map)),
                            bindings: b
                                .bindings
                                .iter()
                                .map(|(p, s)| (p.clone(), s.map_value(map)))
                                .collect(),
                        },
                    )
                })
                .collect(),
        }
    }
}
impl<V> Artifact<V> {
    pub(super) fn map_values<W>(&self, map: &mut impl FnMut(&V) -> W) -> Artifact<W> {
        Artifact {
            definition: self.definition.map_values(map),
            operations: self.operations.clone(),
            types: self.types.clone(),
        }
    }
}
impl<F> AttemptState<F> {
    pub(super) fn map_fault<G>(&self, map: &impl Fn(&F) -> G) -> AttemptState<G> {
        match self {
            Self::Prepared => AttemptState::Prepared,
            Self::Running => AttemptState::Running,
            Self::Available => AttemptState::Available,
            Self::Adopted => AttemptState::Adopted,
            Self::Unknown => AttemptState::Unknown,
            Self::Cancelled => AttemptState::Cancelled,
            Self::Failed(f) => AttemptState::Failed(map(f)),
        }
    }
}
impl<F, I> Reason<F, I> {
    pub(super) fn map_details<G, J>(
        &self,
        fault: &impl Fn(&F) -> G,
        id: &impl Fn(&I) -> J,
    ) -> Reason<G, J> {
        match self {
            Self::PolicyDisabled => Reason::PolicyDisabled,
            Self::DemandRequired => Reason::DemandRequired,
            Self::Paused => Reason::Paused,
            Self::Revoked => Reason::Revoked,
            Self::GuardFalse => Reason::GuardFalse,
            Self::GuardUnavailable => Reason::GuardUnavailable,
            Self::Admission(r) => Reason::Admission(*r),
            Self::Input {
                port,
                producer,
                availability,
            } => Reason::Input {
                port: port.clone(),
                producer: producer.clone(),
                availability: availability.clone(),
            },
            Self::Control { parent, state } => Reason::Control {
                parent: parent.clone(),
                state: state.clone(),
            },
            Self::Attempt { id: i, state } => Reason::Attempt {
                id: id(i),
                state: state.map_fault(fault),
            },
        }
    }
}
