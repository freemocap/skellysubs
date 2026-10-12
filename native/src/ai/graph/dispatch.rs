use super::{model::fault, record_access::RecordAccess, state::Execution, *};
use std::sync::Arc;

pub(super) struct Dispatch {
    pub producer: Arc<Execution>,
    pub consumers: Vec<AttemptId>,
}

impl Engine {
    /// Owner-specific authorization precedes the shared producer decision.
    /// Domain authority is still checked in the final owner transaction.
    pub(super) fn dispatch_owner(
        &self,
        records: &mut impl RecordAccess,
        run: &str,
        node: &str,
        attempt: AttemptId,
    ) -> Result<(Arc<Run>, ExecutionId)> {
        let owner = records.run(run)?;
        let current = owner
            .current
            .get(node)
            .ok_or_else(|| fault(CoreFaultCode::UnknownAttempt, "attempt"))?;
        if !owner.active
            || !owner.permits(node)
            || owner.cancelled.contains(node)
            || *current != attempt
        {
            return Err(fault(CoreFaultCode::InvalidDispatch, "attempt"));
        }
        let row = records.attempt(attempt)?;
        if row.run != run || row.node != node || row.attempt.id != attempt {
            return Err(fault(CoreFaultCode::RecordMismatch, "attempt"));
        }
        if row.attempt.state != AttemptState::Prepared {
            return Err(fault(CoreFaultCode::InvalidDispatch, "attempt"));
        }
        Ok((owner, row.attempt.execution))
    }

    /// The same decision serves replay and durable dispatch. Resident derived
    /// indexes select identities; all selected payloads come from RecordAccess.
    pub(super) fn dispatch(
        &self,
        records: &mut impl RecordAccess,
        execution: ExecutionId,
    ) -> Result<Dispatch> {
        let producer = records.execution(execution)?;
        if producer.work.execution != execution {
            return Err(fault(CoreFaultCode::RecordMismatch, "execution"));
        }
        if producer.dispatched || producer.unknown || producer.outcome.is_some() {
            return Err(fault(CoreFaultCode::InvalidDispatch, "execution"));
        }
        let mut eligible = false;
        let mut consumers = Vec::new();
        self.visit_consumers(records, execution, |owner, row| {
            eligible |= row.eligible_consumer(owner, false);
            if row.attempt.state == AttemptState::Prepared {
                consumers.push(row.attempt.id);
            }
        })?;
        if !eligible {
            return Err(fault(CoreFaultCode::RevokedOrPaused, "execution"));
        }
        let limit = match producer.work.resource {
            Resource::Local => self.state.capacity.local,
            Resource::Provider => self.state.capacity.provider,
        };
        let mut busy = 0;
        for id in self.state.executions.unresolved_ids() {
            if id == execution {
                continue;
            }
            let other = records.execution(id)?;
            if other.work.execution != id || other.unknown || other.outcome.is_some() {
                return Err(fault(CoreFaultCode::RecordMismatch, "execution"));
            }
            if other.dispatched && other.work.resource == producer.work.resource {
                busy += 1;
            }
        }
        if busy >= limit {
            return Err(fault(CoreFaultCode::AdmissionHeld, "execution"));
        }
        Ok(Dispatch {
            producer,
            consumers,
        })
    }
}
