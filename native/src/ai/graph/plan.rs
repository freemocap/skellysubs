use super::{
    model::fault,
    registry::{valid_contract, valid_name},
    *,
};
use std::collections::{BTreeMap, BTreeSet};

/// Validated native structure shared by execution and read-only replay.
/// Contains no handler, provider or invocation capability.
pub(super) struct Plan {
    pub artifact: Artifact,
    pub identity: String,
    pub order: Vec<String>,
}
impl Plan {
    pub fn new(artifact: Artifact) -> Result<Self> {
        valid_contract(&artifact.definition.contract)?;
        if artifact.definition.nodes.len() > 1024 {
            return Err(fault(CoreFaultCode::GraphLimit, "nodes"));
        }
        for contract in artifact.types.keys() {
            valid_contract(contract)?;
        }
        for (contract, operation) in &artifact.operations {
            valid_contract(contract)?;
            valid_name(&operation.implementation)?;
            if *contract != operation.contract {
                return Err(fault(CoreFaultCode::ArtifactMismatch, "operation"));
            }
        }
        for (name, port) in artifact
            .definition
            .inputs
            .iter()
            .chain(&artifact.definition.outputs)
            .chain(
                artifact
                    .operations
                    .values()
                    .flat_map(|op| op.inputs.iter().chain(&op.outputs)),
            )
            .chain(
                artifact
                    .definition
                    .compositions
                    .values()
                    .flat_map(|b| b.source.inputs.iter().chain(&b.source.outputs)),
            )
        {
            valid_name(name)?;
            if !artifact.types.contains_key(&port.contract) {
                return Err(fault(CoreFaultCode::UnknownType, name));
            }
        }
        for (id, node) in &artifact.definition.nodes {
            valid_name(id)?;
            if !artifact.operations.contains_key(&node.operation) {
                return Err(fault(CoreFaultCode::UnknownOperation, id));
            }
        }
        let mut plan = Self {
            artifact,
            identity: String::new(),
            order: Vec::new(),
        };
        plan.order = plan.validate()?;
        plan.identity = plan.artifact.fingerprint()?;
        Ok(plan)
    }
    pub fn artifact(&self) -> &Artifact {
        &self.artifact
    }
    pub(super) fn operation(&self, node: &str) -> &Operation {
        &self.artifact.operations[&self.artifact.definition.nodes[node].operation]
    }
    pub(super) fn values(&self, ports: &Ports, values: &Values) -> Result<()> {
        if values.keys().any(|k| !ports.contains_key(k)) {
            return Err(fault(CoreFaultCode::UnexpectedPort, "values"));
        }
        for (name, port) in ports {
            match values.get(name) {
                None if port.optional => (),
                Some(value) if self.artifact.types[&port.contract].accepts(value) => (),
                _ => return Err(fault(CoreFaultCode::InvalidValue, name)),
            }
        }
        Ok(())
    }
    pub(super) fn source_port(&self, source: &Source) -> Result<Port> {
        let d = &self.artifact.definition;
        match source {
            Source::Input(name) => d
                .inputs
                .get(name)
                .cloned()
                .ok_or_else(|| fault(CoreFaultCode::UnknownInput, name)),
            Source::Output { node, port } => {
                let n = d
                    .nodes
                    .get(node)
                    .ok_or_else(|| fault(CoreFaultCode::UnknownNode, node))?;
                let mut p = self.artifact.operations[&n.operation]
                    .outputs
                    .get(port)
                    .cloned()
                    .ok_or_else(|| fault(CoreFaultCode::UnknownOutput, port))?;
                p.optional |= n.guard.is_some() || n.activation == Activation::Disabled;
                Ok(p)
            }
            Source::Absent(contract) => {
                if !self.artifact.types.contains_key(contract) {
                    return Err(fault(CoreFaultCode::UnknownType, "source"));
                }
                Ok(Port {
                    contract: contract.clone(),
                    optional: true,
                })
            }
            Source::Constant { contract, value } => {
                let shape = self
                    .artifact
                    .types
                    .get(contract)
                    .ok_or_else(|| fault(CoreFaultCode::UnknownType, "source"))?;
                if !shape.accepts(value) {
                    return Err(fault(CoreFaultCode::InvalidConstant, "constant"));
                }
                Ok(Port {
                    contract: contract.clone(),
                    optional: false,
                })
            }
        }
    }
    fn bindings(&self, ports: &Ports, bindings: &BTreeMap<String, Source>) -> Result<()> {
        if ports.keys().ne(bindings.keys()) {
            return Err(fault(CoreFaultCode::BindingSet, "ports"));
        }
        for (name, target) in ports {
            let source = self.source_port(&bindings[name])?;
            if source.contract != target.contract || (source.optional && !target.optional) {
                return Err(fault(CoreFaultCode::IncompatiblePort, name));
            }
        }
        Ok(())
    }
    fn validate(&self) -> Result<Vec<String>> {
        let d = &self.artifact.definition;
        self.bindings(&d.outputs, &d.results)?;
        for boundary in d.compositions.values() {
            self.bindings(&boundary.source.inputs, &boundary.bindings)?;
        }
        for (id, node) in &d.nodes {
            self.bindings(&self.operation(id).inputs, &node.inputs)?;
            let mut seen = BTreeSet::new();
            for parent in &node.after {
                if !d.nodes.contains_key(parent) {
                    return Err(fault(CoreFaultCode::UnknownControlParent, id));
                }
                if !seen.insert(parent) {
                    return Err(fault(CoreFaultCode::DuplicateControl, id));
                }
            }
            if let Some(guard) = &node.guard {
                let port = self.source_port(guard)?;
                if port.optional || self.artifact.types[&port.contract] != Shape::Boolean {
                    return Err(fault(CoreFaultCode::InvalidGuard, id));
                }
            }
        }
        let mut complete = BTreeSet::new();
        let mut order = Vec::new();
        while complete.len() < d.nodes.len() {
            let before = complete.len();
            for (id, node) in &d.nodes {
                if complete.contains(id) {
                    continue;
                }
                let data = node
                    .inputs
                    .values()
                    .chain(node.guard.iter())
                    .filter_map(|s| match s {
                        Source::Output { node, .. } => Some(node),
                        _ => None,
                    });
                if node.after.iter().chain(data).all(|p| complete.contains(p)) {
                    complete.insert(id.clone());
                    order.push(id.clone());
                }
            }
            if before == complete.len() {
                return Err(fault(CoreFaultCode::DependencyCycle, "nodes"));
            }
        }
        Ok(order)
    }
}
