use super::{model::fault, *};

/// Owner-declared ceilings on retained identities, including inactive runs,
/// superseded attempts and retained executions. Eviction does not reset them.
/// These are not eviction rules or
/// byte/RSS limits; checkpoint, history and settlement byte bounds remain separate.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct StateLimits {
    pub runs: usize,
    pub attempts: usize,
    pub executions: usize,
}

/// Counts derived from native identity sets or loaded primary maps, as specified
/// by state_usage (retained) and resident_usage (loaded).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct StateUsage {
    pub runs: usize,
    pub attempts: usize,
    pub executions: usize,
}

impl StateLimits {
    fn check(self, usage: StateUsage) -> Result<()> {
        for (used, limit, code) in [
            (usage.runs, self.runs, CoreFaultCode::RunLimit),
            (usage.attempts, self.attempts, CoreFaultCode::AttemptLimit),
            (
                usage.executions,
                self.executions,
                CoreFaultCode::ExecutionLimit,
            ),
        ] {
            if used > limit {
                return Err(fault(code, "engine"));
            }
        }
        Ok(())
    }
}

impl Engine {
    /// Install admission policy without changing recorded semantics or history.
    /// Durable owners and historical readers must bind this before replay.
    pub fn with_state_limits(mut self, limits: StateLimits) -> Result<Self> {
        limits.check(self.state_usage()?)?;
        self.state_limits = Some(limits);
        Ok(self)
    }

    pub fn state_usage(&self) -> Result<StateUsage> {
        Ok(StateUsage {
            runs: self.state.runs.len(),
            attempts: self.state.attempts.len(),
            executions: self.state.executions.len(),
        })
    }

    /// Check before allocating new records. The outer event transaction also
    /// checks its completed candidate so future allocation paths cannot bypass it.
    pub(super) fn admit_records(
        &self,
        runs: usize,
        attempts: usize,
        executions: usize,
    ) -> Result<()> {
        let Some(limits) = self.state_limits else {
            return Ok(());
        };
        let used = self.state_usage()?;
        let add = |a: usize, b: usize, code| a.checked_add(b).ok_or_else(|| fault(code, "engine"));
        limits.check(StateUsage {
            runs: add(used.runs, runs, CoreFaultCode::RunLimit)?,
            attempts: add(used.attempts, attempts, CoreFaultCode::AttemptLimit)?,
            executions: add(used.executions, executions, CoreFaultCode::ExecutionLimit)?,
        })
    }
}
