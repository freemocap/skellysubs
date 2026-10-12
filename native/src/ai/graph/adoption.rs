use super::{model::fault, record_access::RecordAccess, state::Execution, *};
use std::sync::Arc;

pub(super) struct Adoption {
    pub owner: Arc<Run>,
    pub producer: Arc<Execution>,
}

impl Adoption {
    pub fn values(&self) -> &Values {
        self.producer
            .outcome
            .as_ref()
            .and_then(|r| r.as_ref().ok())
            .expect("validated adoption outcome")
    }
}

impl Engine {
    /// One adoption decision for resident execution and persisted record reads.
    /// The owner transaction still checks current domain authority at commit.
    pub(super) fn adoption(
        &self,
        records: &mut impl RecordAccess,
        run: &str,
        node: &str,
        attempt: AttemptId,
    ) -> Result<Adoption> {
        let owner = records.run(run)?;
        let graph = self
            .graphs
            .get(&owner.artifact)
            .ok_or_else(|| fault(CoreFaultCode::UnknownArtifact, "artifact"))?;
        if !graph.artifact.definition.nodes.contains_key(node) {
            return Err(fault(CoreFaultCode::UnknownNode, "node"));
        }
        if !owner.active || owner.cancelled.contains(node) {
            return Err(fault(CoreFaultCode::Revoked, "node"));
        }
        let current = owner
            .current
            .get(node)
            .ok_or_else(|| fault(CoreFaultCode::UnknownAttempt, "attempt"))?;
        if *current != attempt {
            return Err(fault(CoreFaultCode::InvalidAdoption, "attempt"));
        }
        let row = records.attempt(attempt)?;
        if row.run != run || row.node != node || row.attempt.id != attempt {
            return Err(fault(CoreFaultCode::RecordMismatch, "attempt"));
        }
        if row.attempt.state != AttemptState::Available {
            return Err(fault(CoreFaultCode::InvalidAdoption, "attempt"));
        }
        let producer = records.execution(row.attempt.execution)?;
        if producer.work.execution != row.attempt.execution {
            return Err(fault(CoreFaultCode::RecordMismatch, "execution"));
        }
        if producer
            .outcome
            .as_ref()
            .and_then(|r| r.as_ref().ok())
            .is_none()
        {
            return Err(fault(CoreFaultCode::MissingResult, "execution"));
        }
        Ok(Adoption { owner, producer })
    }
}
