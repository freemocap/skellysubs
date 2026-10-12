use super::plan::Plan;
use super::{
    model::fault,
    registry::{valid_contract, valid_name},
    *,
};
use sha2::{Digest, Sha256};
use std::{collections::BTreeMap, sync::Arc};

pub struct Executable {
    pub(super) plan: Arc<Plan>,
    pub(super) source: Definition,
    pub(super) handlers: BTreeMap<Contract, Handler>,
}
impl Registry {
    pub fn compile(&self, definition: Definition) -> Result<Executable> {
        self.compile_at(definition, 0)
    }
    pub(super) fn compile_at(
        &self,
        mut definition: Definition,
        depth: usize,
    ) -> Result<Executable> {
        let source = definition.clone();
        self.expand(&mut definition, depth)?;
        valid_contract(&definition.contract)?;
        if definition.nodes.len() > 1024 {
            return Err(fault(CoreFaultCode::GraphLimit, "nodes"));
        }
        let mut operations = BTreeMap::new();
        let mut handlers = BTreeMap::new();
        for (id, node) in &definition.nodes {
            valid_name(id)?;
            let (op, handler) = self
                .operations
                .get(&node.operation)
                .ok_or_else(|| fault(CoreFaultCode::UnknownOperation, id))?;
            operations.insert(node.operation.clone(), op.clone());
            handlers.insert(node.operation.clone(), handler.clone());
        }
        let mut types = BTreeMap::new();
        for (name, port) in definition
            .inputs
            .iter()
            .chain(&definition.outputs)
            .chain(
                definition
                    .compositions
                    .values()
                    .flat_map(|b| b.source.inputs.iter().chain(&b.source.outputs)),
            )
            .chain(
                operations
                    .values()
                    .flat_map(|op| op.inputs.iter().chain(&op.outputs)),
            )
        {
            valid_name(name)?;
            let shape = self
                .types
                .get(&port.contract)
                .ok_or_else(|| fault(CoreFaultCode::UnknownType, name))?;
            types.insert(port.contract.clone(), shape.clone());
        }
        // Source constants and guards may use types absent from operation ports.
        for source in definition
            .nodes
            .values()
            .flat_map(|n| n.inputs.values().chain(n.guard.iter()))
            .chain(definition.results.values())
            .chain(
                definition
                    .compositions
                    .values()
                    .flat_map(|b| b.bindings.values()),
            )
        {
            if let Source::Constant { contract, .. } | Source::Absent(contract) = source {
                let shape = self
                    .types
                    .get(contract)
                    .ok_or_else(|| fault(CoreFaultCode::UnknownType, "source"))?;
                types.insert(contract.clone(), shape.clone());
            }
        }
        let artifact = Artifact {
            definition,
            operations,
            types,
        };
        Ok(Executable {
            plan: Arc::new(Plan::new(artifact)?),
            source,
            handlers,
        })
    }
}
impl Executable {
    pub fn artifact(&self) -> &Artifact {
        &self.plan.artifact
    }
    pub fn identity(&self) -> &str {
        &self.plan.identity
    }
    /// Uses the handler bound during compilation, never an independently resolved catalog.
    pub(super) async fn execute(&self, work: &Work, context: InvocationContext) -> Result<Values> {
        if work.artifact != self.plan.identity {
            return Err(fault(CoreFaultCode::ArtifactMismatch, "work"));
        }
        let op = self
            .plan
            .artifact
            .operations
            .get(&work.operation)
            .ok_or_else(|| fault(CoreFaultCode::UnknownOperation, "work"))?;
        self.plan.values(&op.inputs, &work.inputs)?;
        let values = (self.handlers[&work.operation])(context, work.inputs.clone()).await?;
        self.plan.values(&op.outputs, &values)?;
        Ok(values)
    }
}
impl Artifact {
    pub(super) fn fingerprint(&self) -> Result<String> {
        // Shared by compilation and retained-evidence validation. Keep this exact
        // encoding stable: existing artifact identities use it in format 1.
        digest(&(
            &self.definition,
            self.operations.iter().collect::<Vec<_>>(),
            self.types.iter().collect::<Vec<_>>(),
        ))
    }
}
pub(super) fn digest(value: &impl serde::Serialize) -> Result<String> {
    let bytes =
        serde_json::to_vec(value).map_err(|_| fault(CoreFaultCode::Serialization, "artifact"))?;
    Ok(format!("{:x}", Sha256::digest(bytes)))
}
