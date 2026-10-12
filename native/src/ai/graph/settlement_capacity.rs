use super::{encoding::bounded_json, model::fault, *};

/// Owner admission bounds. Settlement bytes include the complete serialized
/// Event::Settle, not only its output values. No evidence is discarded to fit.
#[derive(Clone, Copy)]
pub struct DurableLimits {
    pub record_reads: RecordReadLimits,
    pub state: StateLimits,
    pub checkpoint: CheckpointLimits,
    pub settlement_event_bytes: usize,
    pub history: HistoryLimits,
}

impl DurableLimits {
    pub(super) fn validate(&self) -> Result<()> {
        if self.settlement_event_bytes == 0 {
            return Err(fault(CoreFaultCode::SettlementLimit, "policy"));
        }
        Ok(())
    }

    pub(super) fn validate_event(&self, event: &Event) -> Result<()> {
        if matches!(
            event,
            Event::Settle { .. } | Event::SettleObserved(_) | Event::Observe(_)
        ) {
            bounded_json(
                event,
                self.settlement_event_bytes,
                CoreFaultCode::SettlementLimit,
            )?;
        }
        Ok(())
    }

    pub(super) fn reserve(&self, engine: &Engine, checkpoint: &Checkpoint) -> Result<()> {
        self.reserve_using(engine, checkpoint, &mut &engine.state)
    }

    pub(super) fn reserve_using(
        &self,
        engine: &Engine,
        checkpoint: &Checkpoint,
        records: &mut impl super::record_access::RecordAccess,
    ) -> Result<()> {
        let mut events = engine.journal().len();
        let mut bytes = checkpoint.bytes().len();
        // The first observed event can upgrade the enclosing format. Reserve
        // its header once before exposing any dispatched external effect.
        let mut evidence_header = checkpoint.evidence_header_reservation();
        let mut reserve = |size: usize| -> Result<()> {
            events = events
                .checked_add(1)
                .filter(|n| *n <= self.checkpoint.events)
                .ok_or_else(|| fault(CoreFaultCode::CheckpointEventLimit, "checkpoint"))?;
            bytes = size
                .checked_add(1)
                .and_then(|n| bytes.checked_add(n))
                .filter(|n| *n <= self.checkpoint.bytes)
                .ok_or_else(|| fault(CoreFaultCode::CheckpointByteLimit, "checkpoint"))?;
            Ok(())
        };
        for id in engine.state.executions.unresolved_ids() {
            let ex = records.execution(id)?;
            if ex.dispatched {
                reserve(
                    self.settlement_event_bytes
                        .checked_add(evidence_header)
                        .ok_or_else(|| fault(CoreFaultCode::CheckpointByteLimit, "checkpoint"))?,
                )?;
                evidence_header = 0;
            }
        }
        for run_id in engine.state.runs.keys() {
            let run = records.run(run_id)?;
            if !run.active {
                continue;
            }
            for (node, id) in &run.current {
                let row = records.attempt(*id)?;
                let a = &row.attempt;
                if !run.cancelled.contains(node)
                    && matches!(a.state, AttemptState::Running | AttemptState::Available)
                {
                    let event = Event::Adopt {
                        run: run_id.clone(),
                        node: node.into(),
                        attempt: a.id,
                    };
                    reserve(
                        bounded_json(
                            &event,
                            self.checkpoint.bytes,
                            CoreFaultCode::CheckpointByteLimit,
                        )?
                        .len(),
                    )?;
                }
            }
        }
        if engine.needs_recovery_using(records)? {
            reserve(
                bounded_json(
                    &Event::Recover,
                    self.checkpoint.bytes,
                    CoreFaultCode::CheckpointByteLimit,
                )?
                .len(),
            )?;
        }
        Ok(())
    }
}

impl Engine {
    /// Exactly the fields changed by Recover. An already recovered journal must
    /// remain restartable without consuming another event on every startup.
    pub(super) fn needs_recovery(&self) -> Result<bool> {
        self.needs_recovery_using(&mut &self.state)
    }

    pub(super) fn needs_recovery_using(
        &self,
        records: &mut impl super::record_access::RecordAccess,
    ) -> Result<bool> {
        if self.state.executions.unresolved_ids().len() > 0 {
            return Ok(true);
        }
        for id in self.state.runs.keys() {
            let run = records.run(id)?;
            if !run.paused || run.stepping.is_some() {
                return Ok(true);
            }
            for id in run.current.values() {
                if matches!(
                    records.attempt(*id)?.attempt.state,
                    AttemptState::Prepared | AttemptState::Running
                ) {
                    return Ok(true);
                }
            }
        }
        Ok(false)
    }
}
