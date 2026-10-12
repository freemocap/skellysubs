use super::{
    record_access::RecordAccess,
    state::{AttemptRecord, Execution, RuntimeState},
    *,
};
use std::sync::Arc;

/// Reads within one native event candidate. A record created or changed by
/// this event is authoritative locally; unchanged records MUST use the supplied
/// reader. Missing or failed persisted reads never fall back to resident data.
pub(super) struct StagedAccess<'a, R> {
    pub base: &'a RuntimeState,
    pub candidate: &'a RuntimeState,
    pub records: &'a mut R,
}

impl<R: RecordAccess> RecordAccess for StagedAccess<'_, R> {
    fn run(&mut self, id: &str) -> Result<Arc<Run>> {
        let Some(row) = self.candidate.runs.get_shared(id) else {
            return self.records.run(id);
        };
        if self
            .base
            .runs
            .get_shared(id)
            .is_some_and(|base| Arc::ptr_eq(&base, &row))
        {
            self.records.run(id)
        } else {
            Ok(row)
        }
    }
    fn attempt(&mut self, id: AttemptId) -> Result<Arc<AttemptRecord>> {
        let Some(row) = self.candidate.attempts.get_shared(&id) else {
            return self.records.attempt(id);
        };
        if self
            .base
            .attempts
            .get_shared(&id)
            .is_some_and(|base| Arc::ptr_eq(&base, &row))
        {
            self.records.attempt(id)
        } else {
            Ok(row)
        }
    }
    fn execution(&mut self, id: ExecutionId) -> Result<Arc<Execution>> {
        let Some(row) = self.candidate.executions.get_shared(&id) else {
            return self.records.execution(id);
        };
        if self
            .base
            .executions
            .get_shared(&id)
            .is_some_and(|base| Arc::ptr_eq(&base, &row))
        {
            self.records.execution(id)
        } else {
            Ok(row)
        }
    }
}
