use super::{model::fault, record_access::RecordAccess, state::AttemptRecord, *};
use std::sync::Arc;

impl Engine {
    pub(super) fn read_node(
        &self,
        records: &mut impl RecordAccess,
        run: &str,
        node: &str,
    ) -> Result<Arc<Run>> {
        let owner = records.run(run)?;
        if !self.graphs[&owner.artifact]
            .artifact
            .definition
            .nodes
            .contains_key(node)
        {
            return Err(fault(CoreFaultCode::UnknownNode, "node"));
        }
        Ok(owner)
    }

    /// The immutable association index selects IDs only. Current membership and
    /// eligibility are resolved from typed records, never inferred from the index.
    pub(super) fn visit_consumers(
        &self,
        records: &mut impl RecordAccess,
        execution: ExecutionId,
        mut visit: impl FnMut(&Run, &AttemptRecord),
    ) -> Result<()> {
        for candidate in self.state.attempts.execution_ids(execution) {
            let row = records.attempt(candidate)?;
            if row.attempt.id != candidate || row.attempt.execution != execution {
                return Err(fault(CoreFaultCode::RecordMismatch, "attempt"));
            }
            let owner = records.run(&row.run)?;
            if owner.current.get(&row.node) == Some(&row.attempt.id) {
                visit(&owner, &row);
            }
        }
        Ok(())
    }

    pub(super) fn has_consumer_using(
        &self,
        records: &mut impl RecordAccess,
        execution: ExecutionId,
        allow_paused: bool,
    ) -> Result<bool> {
        let mut eligible = false;
        self.visit_consumers(records, execution, |owner, row| {
            eligible |= row.eligible_consumer(owner, allow_paused);
        })?;
        Ok(eligible)
    }
}
