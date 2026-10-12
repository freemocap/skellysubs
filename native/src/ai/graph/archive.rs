use super::{model::fault, *};
use std::{collections::BTreeSet, sync::Arc};

/// Total retained prefix read during recovery, excluding the active checkpoint.
/// Bytes include immutable snapshots as well as event suffixes; no silent paging.
#[derive(Clone, Copy)]
pub struct HistoryLimits {
    pub bytes: usize,
    pub events: usize,
    pub segments: usize,
}

#[derive(Clone, Copy, Default)]
pub(super) struct HistoryUsage {
    bytes: usize,
    events: usize,
    segments: usize,
}
impl HistoryUsage {
    pub fn add(self, checkpoint: &Checkpoint, limits: HistoryLimits) -> Result<Self> {
        let add = |used: usize, extra: usize, max: usize| {
            used.checked_add(extra)
                .filter(|n| *n <= max)
                .ok_or_else(|| fault(CoreFaultCode::HistoryLimit, "checkpoint"))
        };
        Ok(Self {
            bytes: add(self.bytes, checkpoint.bytes().len(), limits.bytes)?,
            events: add(self.events, checkpoint.suffix_len(), limits.events)?,
            segments: add(self.segments, 1, limits.segments)?,
        })
    }
}

impl Checkpoint {
    /// Iterative, bounded chain audit. Verify each cut against actual native
    /// replay; a deserialized state snapshot is never trusted as executable state.
    /// Handlers are bound for compatibility but are never invoked during replay.
    pub(super) fn replay_history(
        &self,
        graphs: impl IntoIterator<Item = Arc<Executable>>,
        limits: HistoryLimits,
        state: StateLimits,
        store: &mut impl CommitStore,
    ) -> Result<(Engine, HistoryUsage)> {
        if self.archive_parent().is_none() {
            return Ok((self.replay(graphs, state)?, HistoryUsage::default()));
        }
        let (segments, usage) = self.retained_segments(limits, store)?;
        let mut engine = Engine::new(graphs)?.with_state_limits(state)?;
        for segment in segments.iter().rev() {
            segment.replay_into(&mut engine)?;
        }
        self.replay_into(&mut engine)?;
        Ok((engine, usage))
    }

    pub(super) fn retained_segments(
        &self,
        limits: HistoryLimits,
        store: &mut impl HistoryStore,
    ) -> Result<(Vec<Checkpoint>, HistoryUsage)> {
        let mut usage = HistoryUsage::default();
        let mut segments = Vec::new();
        let mut parent = self.archive_parent().cloned();
        let mut seen = BTreeSet::from([self.stamp().checksum.clone()]);
        while let Some(expected) = parent {
            if usage.segments >= limits.segments || !seen.insert(expected.checksum.clone()) {
                return Err(fault(CoreFaultCode::HistoryLimit, "checkpoint"));
            }
            let remaining = limits.bytes.saturating_sub(usage.bytes);
            let bytes = store.read_archive(&expected, remaining)?;
            let segment = Checkpoint::decode(
                &bytes,
                CheckpointLimits {
                    bytes: remaining,
                    events: limits.events.saturating_sub(usage.events),
                },
            )?;
            if segment.stamp() != &expected
                || segment.suffix_len() == 0
                || segment.base_revision() >= segment.stamp().revision
            {
                return Err(fault(CoreFaultCode::HistoryMismatch, "checkpoint"));
            }
            usage = usage.add(&segment, limits)?;
            parent = segment.archive_parent().cloned();
            segments.push(segment);
        }
        Ok((segments, usage))
    }
}
