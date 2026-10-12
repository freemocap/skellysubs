use super::{checkpoint_legacy::InlineState, state::RuntimeState, *};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

// Preserve format-1 field order and encoding for its existing integrity digest.
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct JournalPayload {
    pub format: u32,
    pub engine: String,
    pub artifacts: BTreeMap<String, Artifact>,
    pub events: Vec<Event>,
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Base<S = SavedState> {
    pub revision: u64,
    pub state: S,
    pub archive: Stamp,
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct SnapshotPayload<S = SavedState> {
    pub format: u32,
    pub engine: String,
    pub artifacts: BTreeMap<String, Artifact>,
    pub base: Base<S>,
    pub events: Vec<Event>,
}

/// Formats 5–9 explicitly select the optional base representation. Older payloads
/// retain their exact encoding/checksum; no untagged state decoder is used.
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct EvidencePayload<S = SavedState> {
    pub format: u32,
    pub engine: String,
    pub artifacts: BTreeMap<String, Artifact>,
    pub base_format: Option<u32>,
    pub base: Option<Base<S>>,
    pub events: Vec<Event>,
}

impl<S> EvidencePayload<S> {
    fn map_state(self, convert: impl FnOnce(S) -> SavedState) -> EvidencePayload {
        EvidencePayload {
            format: self.format,
            engine: self.engine,
            artifacts: self.artifacts,
            base_format: self.base_format,
            base: self.base.map(|base| Base {
                revision: base.revision,
                state: convert(base.state),
                archive: base.archive,
            }),
            events: self.events,
        }
    }
}

#[derive(Clone, Serialize)]
#[serde(untagged)]
pub(super) enum SavedState {
    Inline(InlineState),
    Records(Box<RuntimeState>),
    Commitments(super::checkpoint_records::RecordState),
}

impl SavedState {
    fn has_structured_evidence(&self) -> bool {
        match self {
            Self::Inline(saved) => saved.has_structured_evidence(),
            Self::Records(saved) => saved.executions.iter().any(|(_, e)| {
                e.evidence
                    .as_ref()
                    .is_some_and(ExecutionEvidence::has_structured)
            }),
            Self::Commitments(_) => false, // audited against archive replay
        }
    }
    fn has_provisional(&self) -> bool {
        match self {
            Self::Inline(saved) => saved.has_provisional(),
            Self::Records(saved) => saved
                .executions
                .iter()
                .any(|(_, e)| e.evidence.as_ref().is_some_and(|e| e.provisional.is_some())),
            Self::Commitments(_) => false, // audited against archive replay
        }
    }
    fn legacy_valid(&self) -> bool {
        match self {
            Self::Inline(saved) => !saved.has_evidence(),
            Self::Records(saved) => saved.executions.iter().all(|(_, e)| e.evidence.is_none()),
            Self::Commitments(_) => true, // audited against the full archive replay
        }
    }
    pub fn format(&self) -> u32 {
        match self {
            Self::Inline(_) => 2,
            Self::Records(_) => 3,
            Self::Commitments(_) => 4,
        }
    }
    pub fn matches(&self, state: &RuntimeState) -> Result<bool> {
        Ok(match self {
            Self::Inline(saved) => saved.matches(state),
            Self::Records(saved) => saved.as_ref() == state,
            Self::Commitments(saved) => return saved.matches(state),
        })
    }
}

impl<S> SnapshotPayload<S> {
    fn map_state(self, convert: impl FnOnce(S) -> SavedState) -> SnapshotPayload {
        SnapshotPayload {
            format: self.format,
            engine: self.engine,
            artifacts: self.artifacts,
            base: Base {
                revision: self.base.revision,
                state: convert(self.base.state),
                archive: self.base.archive,
            },
            events: self.events,
        }
    }
}

#[derive(Serialize)]
#[serde(untagged)]
pub(super) enum Payload {
    Evidence(Box<EvidencePayload>),
    Snapshot(Box<SnapshotPayload>),
    Journal(JournalPayload),
}
impl Payload {
    pub fn evidence_format(&self) -> u32 {
        match self {
            Self::Evidence(p) => p.format,
            _ => 0,
        }
    }
    pub fn supports_evidence(&self) -> bool {
        matches!(self, Self::Evidence(_))
    }
    pub fn version_valid(&self) -> bool {
        if self.evidence_format() < 9
            && self
                .artifacts()
                .values()
                .any(|artifact| artifact.types.values().any(Shape::needs_format_nine))
        {
            return false;
        }
        if self.evidence_format() < 8 && (self.events().iter().any(|e| matches!(e, Event::Step { .. }))
            || self.base().is_some_and(|b| matches!(&b.state, SavedState::Records(s) if s.runs.iter().any(|(_, r)| r.stepping.is_some())))) {
            return false;
        }
        if self.evidence_format() < 7
            && (self.events().iter().any(Event::has_structured_evidence)
                || self
                    .base()
                    .is_some_and(|b| b.state.has_structured_evidence()))
        {
            return false;
        }
        if self.evidence_format() < 6
            && (self.events().iter().any(|e| e.provisional().is_some())
                || self.base().is_some_and(|b| b.state.has_provisional()))
        {
            return false;
        }
        if !self.supports_evidence()
            && (self
                .events()
                .iter()
                .any(|e| e.evidence_identity().is_some())
                || self.base().is_some_and(|base| !base.state.legacy_valid()))
        {
            return false;
        }
        match self {
            Self::Evidence(p) => {
                matches!(p.format, 5..=9)
                    && p.base_format == p.base.as_ref().map(|b| b.state.format())
            }
            Self::Journal(p) => p.format == 1,
            Self::Snapshot(p) => p.format == p.base.state.format(),
        }
    }
    pub fn engine(&self) -> &str {
        match self {
            Self::Evidence(p) => &p.engine,
            Self::Journal(p) => &p.engine,
            Self::Snapshot(p) => &p.engine,
        }
    }
    pub fn artifacts(&self) -> &BTreeMap<String, Artifact> {
        match self {
            Self::Evidence(p) => &p.artifacts,
            Self::Journal(p) => &p.artifacts,
            Self::Snapshot(p) => &p.artifacts,
        }
    }
    pub fn events(&self) -> &[Event] {
        match self {
            Self::Evidence(p) => &p.events,
            Self::Journal(p) => &p.events,
            Self::Snapshot(p) => &p.events,
        }
    }
    pub fn base(&self) -> Option<&Base> {
        match self {
            Self::Evidence(p) => p.base.as_ref(),
            Self::Journal(_) => None,
            Self::Snapshot(p) => Some(&p.base),
        }
    }
}

#[derive(Serialize)]
pub(super) struct Envelope {
    pub payload: Payload,
    pub checksum: String,
}

/// Select a concrete decoder explicitly. The discriminator reads only version;
/// the selected full decoder rejects every unknown field. In particular, avoid
/// serde's untagged buffering of numeric execution IDs used as JSON map keys.
pub(super) fn decode(bytes: &[u8]) -> Result<Envelope> {
    #[derive(Deserialize)]
    struct Version {
        format: u32,
        base_format: Option<u32>,
    }
    #[derive(Deserialize)]
    struct Header {
        payload: Version,
    }
    #[derive(Deserialize)]
    #[serde(deny_unknown_fields)]
    struct Wire<T> {
        payload: T,
        checksum: String,
    }
    let invalid = |_| super::model::fault(CoreFaultCode::InvalidCheckpoint, "checkpoint");
    let header: Header = serde_json::from_slice(bytes).map_err(invalid)?;
    match header.payload.format {
        1 => {
            let wire: Wire<JournalPayload> = serde_json::from_slice(bytes).map_err(invalid)?;
            Ok(Envelope {
                payload: Payload::Journal(wire.payload),
                checksum: wire.checksum,
            })
        }
        2 => {
            let wire: Wire<SnapshotPayload<InlineState>> =
                serde_json::from_slice(bytes).map_err(invalid)?;
            Ok(Envelope {
                payload: Payload::Snapshot(Box::new(wire.payload.map_state(SavedState::Inline))),
                checksum: wire.checksum,
            })
        }
        3 => {
            let wire: Wire<SnapshotPayload<RuntimeState>> =
                serde_json::from_slice(bytes).map_err(invalid)?;
            Ok(Envelope {
                payload: Payload::Snapshot(Box::new(
                    wire.payload
                        .map_state(|state| SavedState::Records(Box::new(state))),
                )),
                checksum: wire.checksum,
            })
        }
        4 => {
            let wire: Wire<SnapshotPayload<super::checkpoint_records::RecordState>> =
                serde_json::from_slice(bytes).map_err(invalid)?;
            Ok(Envelope {
                payload: Payload::Snapshot(Box::new(
                    wire.payload.map_state(SavedState::Commitments),
                )),
                checksum: wire.checksum,
            })
        }
        5..=9 => {
            let payload = match header.payload.base_format {
                None | Some(4) => {
                    let wire: Wire<EvidencePayload<super::checkpoint_records::RecordState>> =
                        serde_json::from_slice(bytes).map_err(invalid)?;
                    (
                        wire.payload.map_state(SavedState::Commitments),
                        wire.checksum,
                    )
                }
                Some(2) => {
                    let wire: Wire<EvidencePayload<InlineState>> =
                        serde_json::from_slice(bytes).map_err(invalid)?;
                    (wire.payload.map_state(SavedState::Inline), wire.checksum)
                }
                Some(3) => {
                    let wire: Wire<EvidencePayload<RuntimeState>> =
                        serde_json::from_slice(bytes).map_err(invalid)?;
                    (
                        wire.payload
                            .map_state(|state| SavedState::Records(Box::new(state))),
                        wire.checksum,
                    )
                }
                _ => {
                    return Err(super::model::fault(
                        CoreFaultCode::CheckpointVersion,
                        "checkpoint",
                    ));
                }
            };
            Ok(Envelope {
                payload: Payload::Evidence(Box::new(payload.0)),
                checksum: payload.1,
            })
        }
        _ => Err(super::model::fault(
            CoreFaultCode::CheckpointVersion,
            "checkpoint",
        )),
    }
}
