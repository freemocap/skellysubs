//! Materialized protected reads. Scheduling and topology remain native facts.
use super::{encoding::bounded_json, model::fault, record_access::RecordAccess, *};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use ts_rs::TS;

#[derive(Clone, Copy)]
pub struct LiveReadLimits {
    pub export: ExportLimits,
    pub captures: usize,
    /// Entire supplied capture batch, including metadata that is not exported.
    pub capture_bytes: usize,
}

/// Deliberately content-bearing. Never use this as a diagnostic export.
#[derive(Serialize, TS)]
pub struct LiveInspection {
    pub graph: InspectionSnapshot,
    pub previews: BTreeMap<String, AttemptPreview>,
}

#[derive(Clone, Debug, Serialize, Deserialize, TS)]
pub struct PreviewCapture {
    pub session: String,
    pub sequence: String,
    pub text: String,
    pub failure: Option<FaultDisclosure>,
}

#[derive(Clone, Debug, Serialize, Deserialize, TS)]
pub struct AttemptPreview {
    pub attempt: String,
    pub execution: String,
    pub capture: PreviewCapture,
    // Last durably retained TEXT sequence; None means no retained capture.
    pub retained_sequence: Option<String>,
    pub uncommitted: bool,
    // Current consumer may present source text as a running preview.
    pub live: bool,
    // A final report committed, independently of success or adoption.
    pub complete: bool,
}

impl Engine {
    pub(super) fn live_read(
        &self,
        records: &mut impl RecordAccess,
        stamp: &Stamp,
        run: &str,
        captures: &[EvidenceSnapshot],
        limits: LiveReadLimits,
    ) -> Result<LiveInspection> {
        if captures.len() > limits.captures {
            return Err(fault(CoreFaultCode::InspectionLimit, "captures"));
        }
        bounded_json(
            &captures,
            limits.capture_bytes,
            CoreFaultCode::InspectionLimit,
        )?;
        let mut incoming = BTreeMap::new();
        for capture in captures {
            if capture.identity.engine.as_deref() != Some(stamp.engine.as_str())
                || incoming
                    .insert(capture.identity.execution, capture)
                    .is_some()
            {
                return Err(fault(CoreFaultCode::EvidenceIdentity, "execution"));
            }
        }
        let graph = self.project_using(records, stamp, run, limits.export)?;
        let previews = self.previews_using(records, run, &incoming, None)?;
        let read = LiveInspection { graph, previews };
        bounded_json(&read, limits.export.bytes, CoreFaultCode::InspectionLimit)?;
        Ok(read)
    }

    pub(super) fn node_preview_using(
        &self,
        records: &mut impl RecordAccess,
        run: &str,
        node: &str,
        bytes: usize,
    ) -> Result<Option<AttemptPreview>> {
        let preview = self
            .previews_using(records, run, &BTreeMap::new(), Some(node))?
            .remove(node);
        bounded_json(&preview, bytes, CoreFaultCode::InspectionLimit)?;
        Ok(preview)
    }

    fn previews_using(
        &self,
        records: &mut impl RecordAccess,
        run: &str,
        incoming: &BTreeMap<ExecutionId, &EvidenceSnapshot>,
        selected: Option<&str>,
    ) -> Result<BTreeMap<String, AttemptPreview>> {
        let owner = records.run(run)?;
        let artifact = self.graphs[&owner.artifact].artifact();
        let mut previews = BTreeMap::new();
        // Current consumers come from the run itself, never from attempt ordering
        // or an operation-name catalog. Dormant nodes remain in graph.artifact.
        for (node, id) in &owner.current {
            if selected.is_some_and(|selected| selected != node) {
                continue;
            }
            let row = records.attempt(*id)?;
            if row.run != run || row.node != *node || row.attempt.id != *id {
                return Err(fault(CoreFaultCode::RecordMismatch, "attempt"));
            }
            let producer = records.execution(row.attempt.execution)?;
            let evidence = producer.evidence.as_ref();
            let retained = evidence.and_then(|e| e.provisional.as_ref());
            let complete = evidence.is_some_and(|e| e.complete);
            let live = row.eligible_consumer(&owner, true)
                && matches!(row.attempt.state, AttemptState::Running)
                && producer.dispatched
                && !producer.unknown
                && producer.outcome.is_none()
                && !complete;
            let candidate = incoming.get(&row.attempt.execution);
            if let Some(candidate) = candidate
                && (candidate.identity.artifact != producer.work.artifact
                    || candidate.identity.operation != producer.work.operation)
            {
                return Err(fault(CoreFaultCode::EvidenceIdentity, "execution"));
            }
            // Terminal/cancelled/unknown consumers never receive uncommitted text.
            // An older capture cannot replace a newer committed replacement.
            let selected = if live {
                match candidate.and_then(|c| c.provisional.as_ref()) {
                    Some(next) => {
                        if let Some(old) = retained
                            && old.session == next.session
                            && next.sequence < old.sequence
                        {
                            retained
                        } else {
                            super::provisional::validate_next(retained, Some(next))?;
                            Some(next)
                        }
                    }
                    None => retained,
                }
            } else {
                retained
            };
            if let Some(selected) = selected {
                previews.insert(
                    node.clone(),
                    AttemptPreview {
                        attempt: id.0.to_string(),
                        execution: row.attempt.execution.0.to_string(),
                        capture: PreviewCapture {
                            session: selected.session.clone(),
                            sequence: selected.sequence.to_string(),
                            text: selected.text.clone(),
                            failure: selected
                                .failure
                                .as_ref()
                                .map(|f| super::projection::disclose_fault(f, artifact)),
                        },
                        retained_sequence: retained.map(|p| p.sequence.to_string()),
                        uncommitted: retained != Some(selected),
                        live,
                        complete,
                    },
                );
            }
        }
        Ok(previews)
    }
}
