use super::{checkpoint_format::*, compile::digest, model::fault, *};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Stamp {
    pub engine: String,
    pub revision: u64,
    pub checksum: String,
}

#[derive(Clone, Copy)]
pub struct CheckpointLimits {
    pub bytes: usize,
    pub events: usize,
}

/// Validated source-containing checkpoint, never a diagnostics payload.
pub struct Checkpoint {
    pub(super) envelope: Envelope,
    bytes: Vec<u8>,
    stamp: Stamp,
}

impl Checkpoint {
    pub(super) fn evidence_header_reservation(&self) -> usize {
        if self.envelope.payload.supports_evidence() {
            0
        } else if self.archive_parent().is_some() {
            b",\"base_format\":4".len()
        } else {
            b",\"base_format\":null,\"base\":null".len()
        }
    }

    pub fn stamp(&self) -> &Stamp {
        &self.stamp
    }
    pub fn bytes(&self) -> &[u8] {
        &self.bytes
    }

    /// Retained definitions can be inspected without registering old handlers.
    /// This is evidence access, not validation/authorization to resume execution.
    pub fn artifact_ids(&self) -> impl ExactSizeIterator<Item = &str> {
        self.envelope.payload.artifacts().keys().map(String::as_str)
    }

    pub fn inspection_definition(
        &self,
        artifact: &str,
        limits: ExportLimits,
    ) -> Result<DefinitionSnapshot> {
        self.envelope
            .payload
            .artifacts()
            .get(artifact)
            .ok_or_else(|| fault(CoreFaultCode::UnknownArtifact, "artifact"))?
            .inspection_definition(artifact, limits)
    }

    pub(super) fn capture(
        engine: &Engine,
        identity: &str,
        limits: CheckpointLimits,
    ) -> Result<Self> {
        Self::capture_base(engine, identity, None, limits, 0)
    }

    pub fn archive_parent(&self) -> Option<&Stamp> {
        self.envelope.payload.base().map(|base| &base.archive)
    }
    pub fn base_revision(&self) -> u64 {
        self.envelope.payload.base().map_or(0, |base| base.revision)
    }
    pub fn suffix_len(&self) -> usize {
        self.envelope.payload.events().len()
    }

    pub(super) fn capture_next(&self, engine: &Engine, limits: CheckpointLimits) -> Result<Self> {
        Self::capture_base(
            engine,
            &self.stamp.engine,
            self.envelope.payload.base().cloned(),
            limits,
            self.envelope.payload.evidence_format(),
        )
    }

    #[cfg(test)]
    pub(super) fn compacted(&self, engine: &Engine, limits: CheckpointLimits) -> Result<Self> {
        let base = Base {
            revision: engine.revision(),
            state: SavedState::Records(Box::new(engine.state.clone())),
            archive: self.stamp.clone(),
        };
        Self::capture_base(
            engine,
            &self.stamp.engine,
            Some(base),
            limits,
            self.envelope.payload.evidence_format(),
        )
    }

    pub(super) fn compacted_records(
        &self,
        engine: &Engine,
        evidence: &super::record_evidence::RecordEvidence,
        limits: CheckpointLimits,
    ) -> Result<Self> {
        let base = Base {
            revision: engine.revision(),
            state: SavedState::Commitments(super::checkpoint_records::RecordState::capture(
                &engine.state,
                evidence,
            )),
            archive: if self.suffix_len() == 0 {
                self.archive_parent()
                    .ok_or_else(|| fault(CoreFaultCode::HistoryRequired, "checkpoint"))?
                    .clone()
            } else {
                self.stamp.clone()
            },
        };
        Self::capture_base(
            engine,
            &self.stamp.engine,
            Some(base),
            limits,
            self.envelope.payload.evidence_format(),
        )
    }

    pub(super) fn has_inline_records(&self) -> bool {
        self.envelope
            .payload
            .base()
            .is_some_and(|base| base.state.format() < 4)
    }

    fn capture_base(
        engine: &Engine,
        identity: &str,
        base: Option<Base>,
        limits: CheckpointLimits,
        evidence_format: u32,
    ) -> Result<Self> {
        if engine.journal_start != base.as_ref().map_or(0, |base| base.revision) {
            return Err(fault(CoreFaultCode::HistoryRequired, "checkpoint"));
        }
        if engine.journal().len() > limits.events {
            return Err(fault(CoreFaultCode::CheckpointEventLimit, "checkpoint"));
        }
        let artifacts = engine
            .graphs
            .iter()
            .map(|(id, g)| (id.clone(), g.artifact().clone()))
            .collect();
        let new_shapes = engine.graphs.values().any(|graph| {
            graph
                .artifact()
                .types
                .values()
                .any(Shape::needs_format_nine)
        });
        let events = engine.journal().to_vec();
        let payload = if new_shapes
            || evidence_format > 0
            || events.iter().any(|e| e.evidence_identity().is_some())
            || events.iter().any(|e| matches!(e, Event::Step { .. }))
        {
            Payload::Evidence(Box::new(EvidencePayload {
                format: if new_shapes || evidence_format >= 9 {
                    9
                } else if evidence_format >= 8
                    || events.iter().any(|e| matches!(e, Event::Step { .. }))
                {
                    8
                } else if evidence_format >= 7 || events.iter().any(Event::has_structured_evidence)
                {
                    7
                } else if evidence_format >= 6 || events.iter().any(|e| e.provisional().is_some()) {
                    6
                } else {
                    5
                },
                engine: identity.into(),
                artifacts,
                base_format: base.as_ref().map(|b| b.state.format()),
                base,
                events,
            }))
        } else {
            match base {
                None => Payload::Journal(JournalPayload {
                    format: 1,
                    engine: identity.into(),
                    artifacts,
                    events,
                }),
                Some(base) => Payload::Snapshot(Box::new(SnapshotPayload {
                    format: base.state.format(),
                    engine: identity.into(),
                    artifacts,
                    base,
                    events,
                })),
            }
        };
        // Bound serialization before hashing, so a large outcome cannot allocate
        // an unlimited serialized checkpoint merely to discover it exceeds limits.
        super::encoding::bounded_json(&payload, limits.bytes, CoreFaultCode::CheckpointByteLimit)?;
        let checksum = digest(&payload)?;
        let envelope = Envelope { payload, checksum };
        let bytes = super::encoding::bounded_json(
            &envelope,
            limits.bytes,
            CoreFaultCode::CheckpointByteLimit,
        )?;
        // Refuse to commit anything the versioned reader cannot load, including
        // nesting limits. Encoding success alone does not establish readability.
        Self::decode(&bytes, limits)
    }

    pub fn decode(bytes: &[u8], limits: CheckpointLimits) -> Result<Self> {
        if bytes.len() > limits.bytes {
            return Err(fault(CoreFaultCode::CheckpointByteLimit, "checkpoint"));
        }
        let envelope = super::checkpoint_format::decode(bytes)?;
        Self::validated(envelope, bytes.to_vec(), limits)
    }

    fn validated(envelope: Envelope, bytes: Vec<u8>, limits: CheckpointLimits) -> Result<Self> {
        let p = &envelope.payload;
        if !p.version_valid() {
            return Err(fault(CoreFaultCode::CheckpointVersion, "checkpoint"));
        }
        if p.events()
            .iter()
            .filter_map(Event::evidence_identity)
            .any(|id| id.engine.as_deref() != Some(p.engine()))
        {
            return Err(fault(CoreFaultCode::EvidenceIdentity, "engine"));
        }
        if !uuid::Uuid::parse_str(p.engine()).is_ok_and(|id| id.to_string() == p.engine()) {
            return Err(fault(CoreFaultCode::CheckpointIdentity, "checkpoint"));
        }
        if p.events().len() > limits.events {
            return Err(fault(CoreFaultCode::CheckpointEventLimit, "checkpoint"));
        }
        if digest(p)? != envelope.checksum {
            return Err(fault(CoreFaultCode::CheckpointCorrupt, "checkpoint"));
        }
        for (identity, artifact) in p.artifacts() {
            if artifact.fingerprint()? != *identity {
                return Err(fault(
                    CoreFaultCode::CheckpointArtifactMismatch,
                    "artifacts",
                ));
            }
        }
        if let Some(base) = p.base()
            && (base.archive.engine != p.engine()
                || base.archive.revision != base.revision
                || base.revision == 0)
        {
            return Err(fault(CoreFaultCode::HistoryMismatch, "checkpoint"));
        }
        let revision = p
            .base()
            .map_or(0, |base| base.revision)
            .checked_add(p.events().len() as u64)
            .ok_or_else(|| fault(CoreFaultCode::RevisionLimit, "checkpoint"))?;
        let stamp = Stamp {
            engine: p.engine().into(),
            revision,
            checksum: envelope.checksum.clone(),
        };
        Ok(Self {
            envelope,
            bytes,
            stamp,
        })
    }

    pub(super) fn replay(
        &self,
        graphs: impl IntoIterator<Item = Arc<Executable>>,
        limits: StateLimits,
    ) -> Result<Engine> {
        let mut engine = Engine::new(graphs)?.with_state_limits(limits)?;
        self.replay_into(&mut engine)?;
        Ok(engine)
    }

    pub(super) fn replay_into(&self, engine: &mut Engine) -> Result<()> {
        self.replay_observed_into(engine, &mut |_| {})
    }

    pub(super) fn replay_observed_into(
        &self,
        engine: &mut Engine,
        observe: &mut impl FnMut(&Engine),
    ) -> Result<()> {
        if self.envelope.payload.evidence_format() < 8
            && engine
                .state
                .runs
                .iter()
                .any(|(_, run)| run.stepping.is_some())
        {
            return Err(fault(CoreFaultCode::CheckpointVersion, "checkpoint"));
        }
        if self.envelope.payload.evidence_format() < 7
            && engine.state.executions.iter().any(|(_, e)| {
                e.evidence
                    .as_ref()
                    .is_some_and(ExecutionEvidence::has_structured)
            })
        {
            return Err(fault(CoreFaultCode::CheckpointVersion, "checkpoint"));
        }
        if self.envelope.payload.evidence_format() < 6
            && engine
                .state
                .executions
                .iter()
                .any(|(_, e)| e.evidence.as_ref().is_some_and(|e| e.provisional.is_some()))
        {
            return Err(fault(CoreFaultCode::CheckpointVersion, "checkpoint"));
        }
        if !self.envelope.payload.supports_evidence()
            && engine
                .state
                .executions
                .iter()
                .any(|(_, e)| e.evidence.is_some())
        {
            return Err(fault(CoreFaultCode::CheckpointVersion, "checkpoint"));
        }
        if engine.graphs.len() != self.envelope.payload.artifacts().len() {
            return Err(fault(
                CoreFaultCode::CheckpointArtifactMismatch,
                "artifacts",
            ));
        }
        for (id, saved) in self.envelope.payload.artifacts() {
            if engine.graphs.get(id).is_none_or(|g| g.artifact() != saved) {
                return Err(fault(
                    CoreFaultCode::CheckpointArtifactMismatch,
                    "artifacts",
                ));
            }
        }
        if let Some(base) = self.envelope.payload.base() {
            if engine.revision() != base.revision {
                return Err(fault(CoreFaultCode::HistoryRequired, "checkpoint"));
            }
            if !base.state.matches(&engine.state)? {
                return Err(fault(CoreFaultCode::SnapshotMismatch, "checkpoint"));
            }
            engine.rebase();
        } else if engine.revision() != 0 {
            return Err(fault(CoreFaultCode::HistoryMismatch, "checkpoint"));
        }
        for event in self.envelope.payload.events() {
            engine.apply(event.clone())?;
            observe(engine);
        }
        Ok(())
    }
}
