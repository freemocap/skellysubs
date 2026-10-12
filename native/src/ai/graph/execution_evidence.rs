//! Evidence extends execution records; it does not publish values or change topology.
use super::{model::fault, record_access::RecordAccess, *};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EvidenceSnapshot {
    pub identity: InvocationIdentity,
    pub observations: Vec<ResponseEvidence>,
    pub evidence_failure: Option<Fault>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "super::provisional::present"
    )]
    pub provisional: Option<ProvisionalCapture>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ExecutionEvidence {
    pub observations: Vec<ResponseEvidence>,
    pub evidence_failure: Option<Fault>,
    /// A final report was accepted; this says nothing about adoption or success.
    pub complete: bool,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "super::provisional::present"
    )]
    pub provisional: Option<ProvisionalCapture>,
}

impl ExecutionEvidence {
    pub(super) fn has_structured(&self) -> bool {
        self.observations
            .iter()
            .any(ResponseEvidence::has_structured)
    }
}

impl Engine {
    pub(super) fn record_evidence(
        &mut self,
        records: &mut impl RecordAccess,
        identity: &InvocationIdentity,
        observations: &[ResponseEvidence],
        failure: &Option<Fault>,
        provisional: &Option<ProvisionalCapture>,
        complete: bool,
    ) -> Result<()> {
        let producer = records.execution(identity.execution)?;
        if identity.artifact != producer.work.artifact
            || identity.operation != producer.work.operation
        {
            return Err(fault(CoreFaultCode::EvidenceIdentity, "execution"));
        }
        if !producer.dispatched
            || producer.outcome.is_some()
            || producer.evidence.as_ref().is_some_and(|e| e.complete)
            || (complete && producer.unknown)
        {
            return Err(fault(CoreFaultCode::AlreadySettled, "execution"));
        }
        if let Some(previous) = &producer.evidence
            && (!observations.starts_with(&previous.observations)
                || (previous.evidence_failure.is_some()
                    && (failure != &previous.evidence_failure
                        || observations != previous.observations)))
        {
            return Err(fault(CoreFaultCode::EvidenceConflict, "execution"));
        }
        super::provisional::validate_next(
            producer
                .evidence
                .as_ref()
                .and_then(|e| e.provisional.as_ref()),
            provisional.as_ref(),
        )?;
        self.state.executions.load(producer);
        self.state.executions.set_evidence(
            &identity.execution,
            ExecutionEvidence {
                observations: observations.to_vec(),
                evidence_failure: failure.clone(),
                complete,
                provisional: provisional.clone(),
            },
        );
        Ok(())
    }

    /// Protected native evidence, not an automatically redacted inspection export.
    pub fn execution_evidence(&self, execution: ExecutionId) -> Result<Option<ExecutionEvidence>> {
        Ok((&self.state).execution(execution)?.evidence.clone())
    }
}

impl Event {
    pub(super) fn has_structured_evidence(&self) -> bool {
        match self {
            Self::Observe(snapshot) => snapshot
                .observations
                .iter()
                .any(ResponseEvidence::has_structured),
            Self::SettleObserved(report) => report
                .observations
                .iter()
                .any(ResponseEvidence::has_structured),
            _ => false,
        }
    }
    pub(super) fn provisional(&self) -> Option<&ProvisionalCapture> {
        match self {
            Self::Observe(snapshot) => snapshot.provisional.as_ref(),
            Self::SettleObserved(report) => report.provisional.as_ref(),
            _ => None,
        }
    }
    pub(super) fn evidence_identity(&self) -> Option<&InvocationIdentity> {
        match self {
            Self::Observe(snapshot) => Some(&snapshot.identity),
            Self::SettleObserved(report) => Some(&report.identity),
            _ => None,
        }
    }
}

impl DurableEngine {
    /// Borrows the report so a rejected transaction cannot consume the caller's
    /// only evidence. An indeterminate acknowledgement still requires recovery.
    pub fn settle_report(
        &mut self,
        report: &InvocationReport,
        store: &mut impl CommitStore,
    ) -> Result<()> {
        self.apply(Event::SettleObserved(report.clone()), store)
            .map(|_| ())
    }

    pub fn record_observations(
        &mut self,
        snapshot: &EvidenceSnapshot,
        store: &mut impl CommitStore,
    ) -> Result<()> {
        self.apply(Event::Observe(snapshot.clone()), store)
            .map(|_| ())
    }
}
