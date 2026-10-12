//! Domain-independent executable DAGs. No provider, database, UI or product imports.
//! See docs/notes/ai-graph-foundations.md and ai-graph-core-semantics.md.
mod adoption;
mod archive;
mod attempts;
mod checkpoint;
mod checkpoint_format;
mod checkpoint_legacy;
mod checkpoint_records;
mod compile;
mod composition;
mod consumer_reads;
mod dependencies;
mod dispatch;
mod durable;
mod encoding;
mod execution_evidence;
mod executions;
mod fault_codes;
mod history;
mod inspection;
mod invocation;
mod live_read;
mod model;
mod plan;
mod projection;
mod projection_map;
mod provisional;
mod read_budget;
mod record_access;
mod record_evidence;
mod record_store;
mod records;
mod registry;
mod runtime;
mod scheduling;
mod settlement_capacity;
mod staged_access;
mod state;
mod state_limits;
mod stepping;
mod transitions;

pub use archive::HistoryLimits;
pub use checkpoint::{Checkpoint, CheckpointLimits, Stamp};
pub use compile::Executable;
pub use durable::{
    Authority, CommitFailure, CommitIntent, CommitRequest, CommitStore, DurableEngine,
    HistoryStore, RunControl,
};
pub use execution_evidence::{EvidenceSnapshot, ExecutionEvidence};
pub use fault_codes::CoreFaultCode;
pub use history::{
    AttemptPage, HistoricalAttempt, HistoricalInspection, HistoricalLimits, HistoryCursor,
    RunHistory,
};
pub use invocation::{
    BillingBasis, BillingEvidence, EvidenceLimits, EvidenceOmission, EvidenceValue,
    InvocationContext, InvocationIdentity, InvocationReport, ResponseEvidence, UsageEvidence,
    ValidationEvidence,
};
pub use live_read::{AttemptPreview, LiveInspection, LiveReadLimits, PreviewCapture};
pub use model::*;
pub use projection::bindings as inspection_bindings;
pub use projection::{
    ConstantDisclosure, DefinitionSnapshot, ExportLimits, FaultDisclosure, InspectionSnapshot,
    Omission,
};
pub use provisional::{ProvisionalCapture, ProvisionalLimits};
pub use read_budget::RecordReadLimits;
pub use record_store::{RecordChanges, RecordKey, RecordStore, RecordWrite};
pub use registry::{Handler, Registry};
pub use runtime::{
    Attempt, AttemptState, Capacity, Disposition, Engine, Event, Inspection, Invocation, Work,
};
pub use settlement_capacity::DurableLimits;
pub use state::Run;
pub use state_limits::{StateLimits, StateUsage};

#[cfg(test)]
mod tests;
