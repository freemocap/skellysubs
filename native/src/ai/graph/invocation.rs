//! Native invocation capture/persistence data, NOT an inspection/diagnostic format.
//! Adapters must classify/redact response information before recording it. The
//! host must retain the report before announcing durable completion.
use super::{encoding::bounded_json, model::fault, *};
use serde::{Deserialize, Serialize};
use std::{
    collections::BTreeMap,
    sync::{Arc, Mutex},
};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct InvocationIdentity {
    pub engine: Option<String>,
    /// Engine-local producer identity; the host supplies the engine/workspace scope.
    pub execution: ExecutionId,
    pub artifact: String,
    pub operation: Contract,
}

/// Explicit sensitivity handling for adapter-specific response fields. Unknown
/// fields must be Omitted, not optimistically marked as public text.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum EvidenceValue {
    PublicText(String),
    /// A bounded, already-classified tree, never a raw provider body. The adapter
    /// owns sensitivity handling for keys and values, including unknown fields.
    ClassifiedJson(serde_json::Value),
    Integer(i64),
    Boolean(bool),
    Omitted(EvidenceOmission),
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum EvidenceOmission {
    Content,
    Credential,
    Unclassified,
    Truncated,
    Unreadable,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct UsageEvidence {
    pub input_tokens: Option<u64>,
    pub output_tokens: Option<u64>,
    pub total_tokens: Option<u64>,
    /// Identifies the source of these counts; absent counts never mean zero.
    pub provenance: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BillingEvidence {
    /// Decimal source representation, not a floating-point recalculation.
    pub amount: String,
    pub currency: String,
    pub provenance: String,
    pub basis: BillingBasis,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum BillingBasis {
    ReportedCharge,
    Estimate,
    Allowance,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ValidationEvidence {
    pub stage: String,
    pub path: String,
    pub expected: String,
}

/// One observation, in arrival order. Multiple observations preserve partial
/// metadata without overwriting it with a less informative terminal response.
/// Text fields must already be classified/redacted by the owning adapter.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ResponseEvidence {
    pub request_id: Option<String>,
    pub requested_model: Option<String>,
    pub actual_model: Option<String>,
    pub finish_reason: Option<String>,
    pub error_code: Option<String>,
    pub redacted_reason: Option<String>,
    pub status: Option<u16>,
    pub elapsed_ms: Option<u64>,
    pub retry_after_ms: Option<u64>,
    pub usage: Option<UsageEvidence>,
    pub billing: Option<BillingEvidence>,
    pub validation: Option<ValidationEvidence>,
    pub additional: BTreeMap<String, EvidenceValue>,
}

impl ResponseEvidence {
    pub(super) fn has_structured(&self) -> bool {
        self.additional
            .values()
            .any(|value| matches!(value, EvidenceValue::ClassifiedJson(_)))
    }
}

#[derive(Clone, Copy)]
pub struct EvidenceLimits {
    pub observations: usize,
    /// Sum of encoded observation bytes, not a heap-allocation guarantee.
    pub bytes: usize,
}

struct Capture {
    limits: EvidenceLimits,
    bytes: usize,
    observations: Vec<ResponseEvidence>,
    failure: Option<Fault>,
    closed: bool,
    provisional: super::provisional::Capture,
}

/// Callback capability scoped to one claimed producer. Retained clones cannot
/// append after the handler returns. It grants no settlement/adoption authority.
#[derive(Clone)]
pub struct InvocationContext {
    identity: InvocationIdentity,
    capture: Arc<Mutex<Capture>>,
}

pub(super) struct InvocationGuard(InvocationContext);
impl Drop for InvocationGuard {
    fn drop(&mut self) {
        let mut capture = self
            .0
            .capture
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        capture.closed = true;
    }
}

/// Protected persistence data, NOT an inspection DTO. No TS export.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
#[must_use = "retain invocation evidence as well as the output outcome"]
pub struct InvocationReport {
    pub identity: InvocationIdentity,
    pub outcome: Result<Values>,
    pub observations: Vec<ResponseEvidence>,
    /// Separate from the handler outcome so an observation failure cannot erase
    /// the original transport/validation failure or silently become success.
    pub evidence_failure: Option<Fault>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "super::provisional::present"
    )]
    pub provisional: Option<ProvisionalCapture>,
}

impl InvocationContext {
    pub(super) fn new(
        work: &Work,
        engine: Option<String>,
        limits: EvidenceLimits,
        provisional: ProvisionalLimits,
    ) -> Self {
        Self {
            identity: InvocationIdentity {
                engine,
                execution: work.execution,
                artifact: work.artifact.clone(),
                operation: work.operation.clone(),
            },
            capture: Arc::new(Mutex::new(Capture {
                limits,
                bytes: 0,
                observations: Vec::new(),
                failure: None,
                closed: false,
                provisional: super::provisional::Capture::new(provisional),
            })),
        }
    }

    pub fn identity(&self) -> &InvocationIdentity {
        &self.identity
    }

    /// Capture a cumulative prefix for the host to persist. Taking this snapshot
    /// is not a durable acknowledgement; the host must commit it explicitly.
    pub fn snapshot(&self) -> Result<EvidenceSnapshot> {
        let capture = self
            .capture
            .lock()
            .map_err(|_| fault(CoreFaultCode::EvidenceUnavailable, "invocation"))?;
        if capture.closed {
            return Err(fault(CoreFaultCode::InvocationClosed, "invocation"));
        }
        Ok(EvidenceSnapshot {
            identity: self.identity.clone(),
            observations: capture.observations.clone(),
            evidence_failure: capture.failure.clone(),
            provisional: capture.provisional.value.clone(),
        })
    }

    pub(super) fn guard(&self) -> InvocationGuard {
        InvocationGuard(self.clone())
    }

    /// Replaces cumulative protected source text. It cannot satisfy output ports
    /// or acknowledge storage. Copies of this capability close with the handler.
    pub fn provisional_text(&self, text: &str) -> Result<()> {
        let mut capture = self
            .capture
            .lock()
            .map_err(|_| fault(CoreFaultCode::EvidenceUnavailable, "invocation"))?;
        if capture.closed {
            return Err(fault(CoreFaultCode::InvocationClosed, "invocation"));
        }
        capture.provisional.replace(text)
    }

    pub fn observe(&self, evidence: ResponseEvidence) -> Result<()> {
        let mut capture = self
            .capture
            .lock()
            .map_err(|_| fault(CoreFaultCode::EvidenceUnavailable, "invocation"))?;
        if capture.closed {
            return Err(fault(CoreFaultCode::InvocationClosed, "invocation"));
        }
        if let Some(error) = &capture.failure {
            return Err(error.clone());
        }
        let checked = if capture.observations.len() >= capture.limits.observations {
            Err(fault(CoreFaultCode::EvidenceLimit, "observations"))
        } else {
            bounded_json(
                &evidence,
                capture.limits.bytes - capture.bytes,
                CoreFaultCode::EvidenceLimit,
            )
        };
        match checked {
            Ok(bytes) => {
                capture.bytes += bytes.len();
                capture.observations.push(evidence);
                Ok(())
            }
            Err(error) => {
                capture.failure = Some(error.clone());
                Err(error)
            }
        }
    }

    pub(super) fn finish(self, outcome: Result<Values>) -> InvocationReport {
        // No user callback runs while holding this lock. If poisoned, keep the
        // observations already accepted and expose the failure explicitly.
        let (mut capture, poisoned) = match self.capture.lock() {
            Ok(capture) => (capture, false),
            Err(error) => (error.into_inner(), true),
        };
        capture.closed = true;
        if poisoned && capture.failure.is_none() {
            capture.failure = Some(fault(CoreFaultCode::EvidenceUnavailable, "invocation"));
        }
        InvocationReport {
            identity: self.identity,
            outcome: outcome.and_then(|values| {
                match capture.failure.as_ref().or_else(|| {
                    capture
                        .provisional
                        .value
                        .as_ref()
                        .and_then(|p| p.failure.as_ref())
                }) {
                    Some(error) => Err(error.clone()),
                    None => Ok(values),
                }
            }),
            observations: std::mem::take(&mut capture.observations),
            evidence_failure: capture.failure.clone(),
            provisional: capture.provisional.value.take(),
        }
    }
}
