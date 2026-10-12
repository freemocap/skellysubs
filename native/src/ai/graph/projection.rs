use super::{model::fault, *};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use ts_rs::TS;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
pub enum Omission {
    Content,
    Unclassified,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
pub struct ConstantDisclosure {
    pub omitted: Omission,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
pub enum Disclosure<T> {
    Known(T),
    Omitted(Omission),
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
pub struct FaultDisclosure {
    pub code: Disclosure<CoreFaultCode>,
    pub path: Disclosure<String>,
}

#[derive(Clone, Copy)]
pub struct ExportLimits {
    pub bytes: usize,
    pub attempts: usize,
}

/// Immutable structure is inspectable before any run exists. No runtime facts
/// are invented; run snapshots overlay this identical artifact representation.
#[derive(Serialize, TS)]
pub struct DefinitionSnapshot {
    pub protocol: u32,
    pub artifact_id: String,
    pub artifact: Artifact<ConstantDisclosure>,
}

impl Executable {
    pub fn inspection_definition(&self, limits: ExportLimits) -> Result<DefinitionSnapshot> {
        self.artifact()
            .inspection_definition(self.identity(), limits)
    }
}

impl Artifact {
    fn projected_definition(&self, identity: &str) -> DefinitionSnapshot {
        DefinitionSnapshot {
            protocol: 1,
            artifact_id: identity.into(),
            artifact: self.map_values(&mut |_| ConstantDisclosure {
                omitted: Omission::Content,
            }),
        }
    }
    pub(super) fn inspection_definition(
        &self,
        identity: &str,
        limits: ExportLimits,
    ) -> Result<DefinitionSnapshot> {
        let snapshot = self.projected_definition(identity);
        super::encoding::bounded_json(&snapshot, limits.bytes, CoreFaultCode::InspectionLimit)?;
        Ok(snapshot)
    }
}

/// Protocol 1. Structural identifiers are authored public schema, never source
/// content. Runtime values and arbitrary adapter faults are not exposed here.
/// Artifact/attempt/reason types are the same generic types used by execution.
#[derive(Clone, Debug, Serialize, Deserialize, TS)]
pub struct InspectionSnapshot<A = BTreeMap<String, Vec<Attempt<FaultDisclosure, String, String>>>> {
    pub protocol: u32,
    pub engine: String,
    pub revision: String,
    pub run: String,
    pub artifact_id: String,
    pub artifact: Artifact<ConstantDisclosure>,
    pub nodes: BTreeMap<String, Disposition>,
    pub activation: BTreeMap<String, Activation>,
    pub paused: bool,
    pub active: bool,
    pub stepping: Option<String>,
    pub step_available: bool,
    pub attempts: A,
    pub reasons: BTreeMap<String, Vec<Reason<FaultDisclosure, String>>>,
}

pub(super) fn disclose_fault(f: &Fault, artifact: &Artifact) -> FaultDisclosure {
    let code: Option<CoreFaultCode> =
        serde_json::from_value(serde_json::Value::String(f.code.clone())).ok();
    // Only closed machine codes and known structural selectors are public. An
    // arbitrary handler failure is retained internally, with explicit omission
    // here until its adapter supplies a typed sensitivity contract.
    let known_path = [
        "node",
        "nodes",
        "run",
        "engine",
        "execution",
        "attempt",
        "artifact",
        "artifacts",
        "work",
        "source",
        "graph",
        "graphs",
        "ports",
        "values",
        "policy",
        "checkpoint",
        "identifier",
        "event",
        "provisional",
    ]
    .contains(&f.path.as_str())
        || artifact.definition.nodes.contains_key(&f.path)
        || artifact.definition.inputs.contains_key(&f.path)
        || artifact.definition.outputs.contains_key(&f.path)
        || artifact.operations.values().any(|o| {
            o.inputs.contains_key(&f.path)
                || o.outputs.contains_key(&f.path)
                || o.contract.name == f.path
        })
        || artifact.types.keys().any(|t| t.name == f.path);
    FaultDisclosure {
        code: code.map_or(
            Disclosure::Omitted(Omission::Unclassified),
            Disclosure::Known,
        ),
        path: if code.is_some() && known_path {
            Disclosure::Known(f.path.clone())
        } else {
            Disclosure::Omitted(Omission::Unclassified)
        },
    }
}

impl Engine {
    pub(super) fn project(
        &self,
        stamp: &Stamp,
        run: &str,
        limits: ExportLimits,
    ) -> Result<InspectionSnapshot> {
        self.project_using(&mut &self.state, stamp, run, limits)
    }

    pub(super) fn project_using(
        &self,
        records: &mut impl super::record_access::RecordAccess,
        stamp: &Stamp,
        run: &str,
        limits: ExportLimits,
    ) -> Result<InspectionSnapshot> {
        let owner = records.run(run)?;
        if self.revision() != stamp.revision {
            return Err(fault(CoreFaultCode::InspectionRevision, "snapshot"));
        }
        let count = self.state.attempts.ids(run).len();
        if count > limits.attempts {
            return Err(fault(CoreFaultCode::InspectionLimit, "attempts"));
        }
        let artifact = self.graphs[&owner.artifact].artifact();
        let mut attempts = BTreeMap::<String, Vec<_>>::new();
        for id in self.state.attempts.ids(run) {
            let row = records.attempt(*id)?;
            if row.run != run || row.attempt.id != *id {
                return Err(fault(CoreFaultCode::RecordMismatch, "attempt"));
            }
            attempts
                .entry(row.node.clone())
                .or_default()
                .push(project_attempt(&row.attempt, artifact));
        }
        self.project_with_attempts_using(
            records,
            &stamp.engine,
            stamp.revision,
            run,
            attempts,
            limits,
        )
    }

    pub(super) fn project_with_attempts<A: Serialize>(
        &self,
        engine: &str,
        revision: u64,
        run: &str,
        attempts: A,
        limits: ExportLimits,
    ) -> Result<InspectionSnapshot<A>> {
        self.project_with_attempts_using(&mut &self.state, engine, revision, run, attempts, limits)
    }

    fn project_with_attempts_using<A: Serialize>(
        &self,
        records: &mut impl super::record_access::RecordAccess,
        engine: &str,
        revision: u64,
        run: &str,
        attempts: A,
        limits: ExportLimits,
    ) -> Result<InspectionSnapshot<A>> {
        let owner = records.run(run)?;
        let view = self.inspect_with_attempts_using(records, run, &owner, ())?;
        if view.revision != revision {
            return Err(fault(CoreFaultCode::InspectionRevision, "snapshot"));
        }
        let definition = self.graphs[&owner.artifact]
            .artifact()
            .projected_definition(&owner.artifact);
        let failure = |f: &Fault| disclose_fault(f, view.artifact);
        let snapshot = InspectionSnapshot {
            protocol: 1,
            engine: engine.into(),
            revision: revision.to_string(),
            run: run.into(),
            artifact_id: definition.artifact_id,
            artifact: definition.artifact,
            nodes: view.nodes,
            activation: view.activation.clone(),
            paused: owner.paused,
            active: owner.active,
            stepping: view.stepping,
            step_available: view.step_available,
            attempts,
            reasons: view
                .reasons
                .iter()
                .map(|(id, reasons)| {
                    (
                        id.clone(),
                        reasons
                            .iter()
                            .map(|r| r.map_details(&failure, &|id: &AttemptId| id.0.to_string()))
                            .collect(),
                    )
                })
                .collect(),
        };
        super::encoding::bounded_json(&snapshot, limits.bytes, CoreFaultCode::InspectionLimit)?;
        Ok(snapshot)
    }
}

pub(super) fn project_attempt(
    a: &Attempt,
    artifact: &Artifact,
) -> Attempt<FaultDisclosure, String, String> {
    Attempt {
        id: a.id.0.to_string(),
        execution: a.execution.0.to_string(),
        state: a.state.map_fault(&|f| disclose_fault(f, artifact)),
        acquisition: a.acquisition,
    }
}

pub fn bindings() -> String {
    let config = ts_rs::Config::default();
    let declarations = [
        Contract::decl(&config),
        Port::decl(&config),
        Shape::decl(&config),
        Resource::decl(&config),
        Reuse::decl(&config),
        Activation::decl(&config),
        Operation::decl(&config),
        Source::<ConstantDisclosure>::decl(&config),
        Node::<ConstantDisclosure>::decl(&config),
        Definition::<ConstantDisclosure>::decl(&config),
        Boundary::<ConstantDisclosure>::decl(&config),
        Artifact::<ConstantDisclosure>::decl(&config),
        Fault::decl(&config),
        AttemptId::decl(&config),
        ExecutionId::decl(&config),
        Acquisition::decl(&config),
        Availability::decl(&config),
        Disposition::decl(&config),
        AttemptState::<FaultDisclosure>::decl(&config),
        Attempt::<FaultDisclosure, String, String>::decl(&config),
        Reason::<FaultDisclosure, String>::decl(&config),
        CoreFaultCode::decl(&config),
        Omission::decl(&config),
        ConstantDisclosure::decl(&config),
        Disclosure::<String>::decl(&config),
        FaultDisclosure::decl(&config),
        DefinitionSnapshot::decl(&config),
        <InspectionSnapshot>::decl(&config),
        PreviewCapture::decl(&config),
        AttemptPreview::decl(&config),
        LiveInspection::decl(&config),
        HistoryCursor::decl(&config),
        HistoricalAttempt::decl(&config),
        AttemptPage::decl(&config),
        RunHistory::decl(&config),
        serde_json::Value::decl(&config),
    ];
    format!(
        "// Generated by npm run contracts from native AI graph types. Do not edit.\n{}\n",
        declarations
            .iter()
            .map(|d| format!("export {d}"))
            .collect::<Vec<_>>()
            .join("\n")
    )
}
