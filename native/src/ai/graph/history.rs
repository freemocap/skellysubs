use super::{model::fault, *};
use serde::{Deserialize, Serialize};
use ts_rs::TS;

/// Chronological native state changes. Topology and overlays are projected
/// by the same exporter as live inspection, without executable handlers.
#[derive(Serialize, Deserialize, TS)]
pub struct RunHistory {
    pub engine: String,
    pub run: String,
    pub current_revision: String,
    pub frames: Vec<InspectionSnapshot>,
    pub before: Option<String>,
}

/// Independent archive input and resident-record reconstruction ceilings.
#[derive(Clone, Copy)]
pub struct HistoricalLimits {
    pub history: HistoryLimits,
    pub state: StateLimits,
}

/// Protocol 1 cursors select an immutable logical cut, not a storage segment.
/// IDs and revisions are canonical decimal strings, never lossy JS numbers.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(deny_unknown_fields)]
pub struct HistoryCursor {
    pub protocol: u32,
    pub engine: String,
    pub run: String,
    pub artifact: String,
    pub revision: String,
    pub after_attempt: String,
}

#[derive(Serialize, TS)]
pub struct HistoricalAttempt {
    pub node: String,
    pub attempt: Attempt<FaultDisclosure, String, String>,
}

/// Ascending native attempt-ID order. total is the entire run at this cut;
/// offset counts earlier records. next=None explicitly marks the final page.
#[derive(Serialize, TS)]
pub struct AttemptPage {
    pub total: String,
    pub offset: String,
    pub records: Vec<HistoricalAttempt>,
    pub next: Option<HistoryCursor>,
}

/// A frozen, read-only native replay. No handlers, mutable engine, invocation,
/// adoption or recovery command are exposed. Reads do not pause historical runs.
pub struct HistoricalInspection {
    engine: Engine,
    identity: String,
}

impl Checkpoint {
    pub fn run_history(
        &self,
        run: &str,
        before: Option<u64>,
        count: usize,
        limits: HistoricalLimits,
        export: ExportLimits,
        store: &mut impl HistoryStore,
    ) -> Result<RunHistory> {
        use std::collections::VecDeque;
        if count == 0 || before.is_some_and(|cut| cut == 0 || cut > self.stamp().revision) {
            return Err(fault(CoreFaultCode::HistoryCursor, "run"));
        }
        let mut engine = Engine::recorded(self.envelope.payload.artifacts(), limits.state)?;
        let (segments, _) = self.retained_segments(limits.history, store)?;
        let mut frames = VecDeque::new();
        let mut bytes = 0;
        let mut omitted = false;
        let mut previous = None;
        let mut failure = None;
        let mut observe = |engine: &Engine| {
            if failure.is_some() || before.is_some_and(|cut| engine.revision() >= cut) {
                return;
            }
            let Some(owner) = engine.state.runs.get(run) else {
                return;
            };
            let observed = (|| -> Result<()> {
                let view = engine.inspect(run)?;
                let current: Vec<_> = view
                    .attempts
                    .iter()
                    .filter_map(|(node, attempts)| {
                        attempts
                            .last()
                            .map(|attempt| (node.clone(), (*attempt).clone()))
                    })
                    .collect();
                let key = (
                    view.nodes,
                    view.reasons,
                    view.activation.clone(),
                    current,
                    owner.paused,
                    owner.active,
                    owner.stepping.clone(),
                    view.step_available,
                );
                if previous.as_ref() == Some(&key) {
                    return Ok(());
                }
                previous = Some(key);
                let snapshot = engine.project(
                    &Stamp {
                        engine: self.stamp().engine.clone(),
                        revision: engine.revision(),
                        checksum: String::new(),
                    },
                    run,
                    export,
                )?;
                let size = super::encoding::bounded_json(
                    &snapshot,
                    export.bytes,
                    CoreFaultCode::InspectionLimit,
                )?
                .len();
                frames.push_back((snapshot, size));
                bytes += size;
                while frames.len() > count || bytes > export.bytes / 2 {
                    if frames.len() == 1 {
                        break;
                    }
                    let (_, size) = frames.pop_front().unwrap();
                    bytes -= size;
                    omitted = true;
                }
                Ok(())
            })();
            if let Err(cause) = observed {
                failure = Some(cause);
            }
        };
        for segment in segments.iter().rev() {
            segment.replay_observed_into(&mut engine, &mut observe)?;
        }
        self.replay_observed_into(&mut engine, &mut observe)?;
        if let Some(cause) = failure {
            return Err(cause);
        }
        engine.run(run)?;
        let history = RunHistory {
            engine: self.stamp().engine.clone(),
            run: run.into(),
            current_revision: self.stamp().revision.to_string(),
            before: if omitted {
                frames.front().map(|(frame, _)| frame.revision.clone())
            } else {
                None
            },
            frames: frames.into_iter().map(|(frame, _)| frame).collect(),
        };
        super::encoding::bounded_json(&history, export.bytes, CoreFaultCode::InspectionLimit)?;
        Ok(history)
    }

    /// Audit formats 1/2/3 with the shared version-1 transition semantics,
    /// capturing the requested logical cut. This bounds input history but still
    /// replays all records and holds all native state; it is not cold-state paging.
    pub fn historical_inspection(
        &self,
        revision: u64,
        limits: HistoricalLimits,
        store: &mut impl HistoryStore,
    ) -> Result<HistoricalInspection> {
        if revision > self.stamp().revision {
            return Err(fault(CoreFaultCode::InspectionRevision, "checkpoint"));
        }
        let mut engine = Engine::recorded(self.envelope.payload.artifacts(), limits.state)?;
        let mut selected = (revision == 0).then(|| engine.clone());
        let (segments, _) = self.retained_segments(limits.history, store)?;
        let mut capture = |engine: &Engine| {
            if engine.revision() == revision {
                selected = Some(engine.clone());
            }
        };
        for segment in segments.iter().rev() {
            segment.replay_observed_into(&mut engine, &mut capture)?;
        }
        self.replay_observed_into(&mut engine, &mut capture)?;
        let engine = selected.ok_or_else(|| fault(CoreFaultCode::HistoryRequired, "checkpoint"))?;
        Ok(HistoricalInspection {
            engine,
            identity: self.stamp().engine.clone(),
        })
    }
}

/// Producer facts at an immutable cut, independent of consumer cancellation/adoption.
pub struct ProducerReceipt {
    pub dispatched: bool,
    pub unknown: bool,
    pub outcome: Option<Result<()>>,
}
impl HistoricalInspection {
    /// Protected source values at this exact cut. This is deliberate private
    /// inspection, not the content-free automatic graph projection.
    pub fn execution_content(
        &self,
        execution: ExecutionId,
    ) -> Result<(Work, Option<Result<Values>>)> {
        let producer = self
            .engine
            .state
            .executions
            .get(&execution)
            .ok_or_else(|| fault(CoreFaultCode::UnknownExecution, "execution"))?;
        Ok((producer.work.clone(), producer.outcome.clone()))
    }
    pub fn producer_receipt(&self, execution: ExecutionId) -> Result<ProducerReceipt> {
        let producer = self
            .engine
            .state
            .executions
            .get(&execution)
            .ok_or_else(|| fault(CoreFaultCode::UnknownExecution, "execution"))?;
        Ok(ProducerReceipt {
            dispatched: producer.dispatched,
            unknown: producer.unknown,
            outcome: producer
                .outcome
                .as_ref()
                .map(|v| v.as_ref().map(|_| ()).map_err(Clone::clone)),
        })
    }

    /// The same bounded native projection used by live inspection. No executable
    /// capability is needed to inspect an unavailable or superseded artifact.
    pub fn snapshot(&self, run: &str, limits: ExportLimits) -> Result<InspectionSnapshot> {
        self.engine.project(
            &Stamp {
                engine: self.identity.clone(),
                revision: self.engine.revision(),
                checksum: String::new(),
            },
            run,
            limits,
        )
    }
    pub fn execution_evidence(&self, execution: ExecutionId) -> Result<Option<ExecutionEvidence>> {
        self.engine.execution_evidence(execution)
    }

    pub fn page(
        &self,
        run: &str,
        cursor: Option<&HistoryCursor>,
        limits: ExportLimits,
    ) -> Result<InspectionSnapshot<AttemptPage>> {
        if limits.attempts == 0 {
            return Err(fault(CoreFaultCode::InspectionLimit, "attempts"));
        }
        let owner = self.engine.run(run)?;
        let expected = HistoryCursor {
            protocol: 1,
            engine: self.identity.clone(),
            run: run.into(),
            artifact: owner.artifact.clone(),
            revision: self.engine.revision().to_string(),
            after_attempt: String::new(),
        };
        let after = if let Some(cursor) = cursor {
            let mut identity = cursor.clone();
            identity.after_attempt.clear();
            if identity != expected {
                return Err(fault(CoreFaultCode::HistoryCursor, "attempt"));
            }
            let id: u64 = cursor
                .after_attempt
                .parse()
                .map_err(|_| fault(CoreFaultCode::HistoryCursor, "attempt"))?;
            if id.to_string() != cursor.after_attempt {
                return Err(fault(CoreFaultCode::HistoryCursor, "attempt"));
            }
            let record = self
                .engine
                .state
                .attempts
                .get(&AttemptId(id))
                .filter(|record| record.run == run)
                .ok_or_else(|| fault(CoreFaultCode::HistoryCursor, "attempt"))?;
            Some(record.attempt.id)
        } else {
            None
        };
        let ids = self.engine.state.attempts.ids(run);
        let total = ids.len();
        let offset = after.map_or(0, |id| ids.partition_point(|candidate| *candidate <= id));
        let artifact = self.engine.graphs[&owner.artifact].artifact();
        let mut last = None;
        let records: Vec<_> = ids[offset..]
            .iter()
            .take(limits.attempts)
            .map(|id| &self.engine.state.attempts[id])
            .map(|record| {
                last = Some(record.attempt.id);
                HistoricalAttempt {
                    node: record.node.clone(),
                    attempt: super::projection::project_attempt(&record.attempt, artifact),
                }
            })
            .collect();
        let next = (offset + records.len() < total).then(|| HistoryCursor {
            after_attempt: last
                .expect("nonempty page before continuation")
                .0
                .to_string(),
            ..expected
        });
        self.engine.project_with_attempts(
            &self.identity,
            self.engine.revision(),
            run,
            AttemptPage {
                total: total.to_string(),
                offset: offset.to_string(),
                records,
                next,
            },
            limits,
        )
    }
}
