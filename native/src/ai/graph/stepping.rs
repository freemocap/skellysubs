//! A step grants one node permission while retaining the run's pause.
use super::{model::fault, record_access::RecordAccess, *};

impl Engine {
    pub(super) fn step_candidate(
        &self,
        records: &mut impl RecordAccess,
        owner: &Run,
    ) -> Result<Option<String>> {
        if !owner.active || !owner.paused || owner.stepping.is_some() {
            return Ok(None);
        }
        let states = self.states_using(records, owner)?;
        if states.values().any(|state| *state == Disposition::Running) {
            return Ok(None);
        }
        // Complete already available work before starting another invocation.
        for eligible in [
            Disposition::Available,
            Disposition::Prepared,
            Disposition::Paused,
        ] {
            if let Some((node, _)) = states.iter().find(|(_, state)| **state == eligible) {
                return Ok(Some(node.clone()));
            }
        }
        Ok(None)
    }

    pub(super) fn step(&mut self, records: &mut impl RecordAccess, run: &str) -> Result<()> {
        let owner = records.run(run)?;
        let node = self
            .step_candidate(records, &owner)?
            .ok_or_else(|| fault(CoreFaultCode::InvalidStep, "run"))?;
        self.state.runs.load(run.into(), owner);
        self.state.runs.get_mut(run).unwrap().stepping = Some(node);
        Ok(())
    }
}
