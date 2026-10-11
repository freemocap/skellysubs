use super::{model::fault, read_budget::BudgetedStore, *};
use std::sync::Arc;

/// Authoritative resident run controls, without inspecting attempts or outputs.
pub struct RunControl {
    pub active: bool,
    pub paused: bool,
}

/// Captured authority to compare with the owner's CURRENT state in its transaction.
pub struct Authority<'a> {
    pub run: &'a str,
    pub node: Option<&'a str>,
    pub artifact: &'a str,
    pub scope: &'a str,
}

pub enum CommitIntent<'a> {
    Record,
    /// Retain these exact source bytes immutably in the SAME transaction that
    /// installs next. The old stamp must match both expected and next's parent.
    Compact {
        archive: &'a Checkpoint,
    },
    Begin {
        authority: Authority<'a>,
        inputs: &'a Values,
    },
    Dispatch {
        authority: Authority<'a>,
        work: &'a Work,
        attempt: AttemptId,
    },
    Adopt {
        authority: Authority<'a>,
        execution: ExecutionId,
        attempt: AttemptId,
        values: &'a Values,
    },
}

#[derive(Debug)]
pub enum CommitFailure {
    /// Adapter guarantees no checkpoint or domain effects were committed.
    Rejected(Fault),
    /// Commit may have happened. Reload storage before any more effects.
    Indeterminate(Fault),
}

pub struct CommitRequest<'a> {
    pub expected: Option<&'a Stamp>,
    pub next: &'a Checkpoint,
    pub intent: CommitIntent<'a>,
    pub records: RecordChanges<'a>,
}

/// One owner transaction: compare stamp, check current authority, publish Adopt
/// values, upsert every supplied native record, replace checkpoint, commit.
/// Err(Rejected) MUST roll back all effects, including partially written records.
/// Implementations must serialize with owner mutation/reset and workspace locking.
pub trait CommitStore: HistoryStore + RecordStore {
    fn commit(&mut self, request: CommitRequest<'_>) -> std::result::Result<(), CommitFailure>;
}

/// Read-only archive capability, separate from authority to publish or commit.
pub trait HistoryStore {
    /// Check the stored length against max_bytes BEFORE allocating/reading its
    /// body. A missing, changed, oversized or unreadable archive is an error.
    fn read_archive(&mut self, expected: &Stamp, max_bytes: usize) -> Result<Vec<u8>>;
}

/// Durable effect boundary. No mutable engine access or cloning; invocations are
/// returned only after dispatch intent has committed in the owner's transaction.
pub struct DurableEngine {
    engine: Engine,
    evidence: super::record_evidence::RecordEvidence,
    checkpoint: Checkpoint,
    limits: DurableLimits,
    poisoned: bool,
    history_usage: super::archive::HistoryUsage,
}

impl DurableEngine {
    /// Current checkpoint suffix footprint. Compaction policy is host-owned;
    /// these counters never reinterpret run or attempt state.
    pub fn checkpoint_usage(&self) -> (usize, usize) {
        (self.checkpoint.bytes().len(), self.engine.journal().len())
    }

    /// Original protected producer inputs, read through committed integrity
    /// evidence. Hosts use these for current authority, never current settings.
    pub fn read_execution_work(
        &self,
        execution: ExecutionId,
        store: &mut impl RecordStore,
    ) -> Result<Work> {
        self.check_live()?;
        let store = &mut BudgetedStore::new(store, self.limits.record_reads);
        let mut records = super::record_access::StoredAccess {
            evidence: &self.evidence,
            max_bytes: self.limits.checkpoint.bytes,
            store,
        };
        Ok(
            super::record_access::RecordAccess::execution(&mut records, execution)?
                .work
                .clone(),
        )
    }
    /// Protected native evidence read against the current committed stamp.
    pub fn read_execution_evidence(
        &self,
        execution: ExecutionId,
        store: &mut impl RecordStore,
    ) -> Result<Option<ExecutionEvidence>> {
        self.check_live()?;
        let store = &mut BudgetedStore::new(store, self.limits.record_reads);
        let mut records = super::record_access::StoredAccess {
            evidence: &self.evidence,
            max_bytes: self.limits.checkpoint.bytes,
            store,
        };
        Ok(
            super::record_access::RecordAccess::execution(&mut records, execution)?
                .evidence
                .clone(),
        )
    }

    pub fn create(
        graphs: impl IntoIterator<Item = Arc<Executable>>,
        limits: DurableLimits,
        store: &mut impl CommitStore,
    ) -> Result<Self> {
        Self::create_initial(graphs, limits, None, store)
    }

    /// Commit a new engine and its first Begin in one owner transaction.
    /// No work is dispatched. Only Event::Begin is accepted. On any error no
    /// host is returned; resolve the owner's receipt/checkpoint before retrying
    /// because a failed commit acknowledgment may still represent a commit.
    pub fn create_with_run(
        graphs: impl IntoIterator<Item = Arc<Executable>>,
        limits: DurableLimits,
        begin: Event,
        store: &mut impl CommitStore,
    ) -> Result<Self> {
        if !matches!(begin, Event::Begin { .. }) {
            return Err(fault(CoreFaultCode::InvalidValue, "initial_event"));
        }
        Self::create_initial(graphs, limits, Some(begin), store)
    }

    fn create_initial(
        graphs: impl IntoIterator<Item = Arc<Executable>>,
        limits: DurableLimits,
        begin: Option<Event>,
        store: &mut impl CommitStore,
    ) -> Result<Self> {
        limits.validate()?;
        let mut engine = Engine::new(graphs)?.with_state_limits(limits.state)?;
        let intent = match &begin {
            Some(
                event @ Event::Begin {
                    run,
                    artifact,
                    scope,
                    inputs,
                    ..
                },
            ) => {
                limits.validate_event(event)?;
                engine.apply(event.clone())?;
                CommitIntent::Begin {
                    authority: Authority {
                        run,
                        node: None,
                        artifact,
                        scope,
                    },
                    inputs,
                }
            }
            None => CommitIntent::Record,
            Some(_) => return Err(fault(CoreFaultCode::InvalidValue, "initial_event")),
        };
        let checkpoint = Checkpoint::capture(
            &engine,
            &uuid::Uuid::new_v4().to_string(),
            limits.checkpoint,
        )?;
        limits.reserve(&engine, &checkpoint)?;
        let evidence = super::record_evidence::RecordEvidence::from_state(
            &engine.state,
            checkpoint.stamp(),
            limits.checkpoint.bytes,
        )?;
        store
            .commit(CommitRequest {
                expected: None,
                next: &checkpoint,
                intent,
                records: RecordChanges::between(None, &engine.state),
            })
            .map_err(commit_fault)?;
        Ok(Self {
            engine,
            evidence,
            checkpoint,
            limits,
            poisoned: false,
            history_usage: super::archive::HistoryUsage::default(),
        })
    }

    pub fn recover(
        checkpoint: Checkpoint,
        graphs: impl IntoIterator<Item = Arc<Executable>>,
        limits: DurableLimits,
        store: &mut impl CommitStore,
    ) -> Result<Self> {
        let store = &mut BudgetedStore::new(store, limits.record_reads);
        limits.validate()?;
        let (engine, history_usage) =
            checkpoint.replay_history(graphs, limits.history, limits.state, store)?;
        super::record_store::verify(
            &engine.state,
            checkpoint.stamp(),
            limits.checkpoint.bytes,
            store,
        )?;
        let evidence = super::record_evidence::RecordEvidence::from_state(
            &engine.state,
            checkpoint.stamp(),
            limits.checkpoint.bytes,
        )?;
        let mut host = Self {
            engine,
            evidence,
            checkpoint,
            limits,
            poisoned: false,
            history_usage,
        };
        if host.engine.needs_recovery()? {
            host.apply(Event::Recover, store)?;
        } else {
            // Recheck caller limits even when recovery is already a fixed point.
            let checked = Checkpoint::decode(host.checkpoint.bytes(), limits.checkpoint)?;
            limits.reserve(&host.engine, &checked)?;
        }
        Ok(host)
    }

    pub fn stamp(&self) -> &Stamp {
        self.checkpoint.stamp()
    }
    /// Owner policy only; changing read admission does not alter graph history.
    pub fn set_record_read_limits(&mut self, limits: RecordReadLimits) -> Result<()> {
        self.check_live()?;
        self.limits.record_reads = limits;
        Ok(())
    }
    pub fn state_usage(&self) -> Result<StateUsage> {
        self.check_live()?;
        self.engine.state_usage()
    }
    /// Loaded primary payload counts, separate from logical retained records.
    pub fn resident_usage(&self) -> Result<StateUsage> {
        self.check_live()?;
        Ok(self.engine.state.resident_usage())
    }

    /// Archive the source-containing suffix, then release every primary payload.
    /// Identities, indexes and integrity commitments remain. No durable record is
    /// deleted. Call read_inspection/read_outputs after eviction.
    pub fn evict_records(&mut self, store: &mut impl CommitStore) -> Result<()> {
        self.check_live()?;
        self.compact(store)?;
        self.engine.state.evict();
        Ok(())
    }
    pub fn inspection_snapshot(
        &self,
        run: &str,
        limits: ExportLimits,
    ) -> Result<InspectionSnapshot> {
        self.check_live()?;
        self.engine.project(self.stamp(), run, limits)
    }
    pub fn run_control(&self, run: &str) -> Result<RunControl> {
        self.check_live()?;
        self.engine.state.require_resident()?;
        let run = self.engine.run(run)?;
        Ok(RunControl {
            active: run.active,
            paused: run.paused,
        })
    }
    pub fn inspect(&self, run: &str) -> Result<Inspection<'_>> {
        self.check_live()?;
        self.engine.inspect(run)
    }
    pub fn outputs(&self, run: &str) -> Result<Option<Values>> {
        self.check_live()?;
        self.engine.outputs(run)
    }

    /// Stamp-bound persisted reads, with the same native projection and disclosure
    /// policy as resident inspection. A read failure returns no partial snapshot.
    pub fn read_inspection(
        &self,
        run: &str,
        limits: ExportLimits,
        store: &mut impl RecordStore,
    ) -> Result<InspectionSnapshot> {
        let store = &mut BudgetedStore::new(store, self.limits.record_reads);
        self.check_live()?;
        self.engine.project_using(
            &mut super::record_access::StoredAccess {
                evidence: &self.evidence,
                max_bytes: self.limits.checkpoint.bytes,
                store,
            },
            self.stamp(),
            run,
            limits,
        )
    }

    pub fn read_outputs(&self, run: &str, store: &mut impl RecordStore) -> Result<Option<Values>> {
        let store = &mut BudgetedStore::new(store, self.limits.record_reads);
        self.check_live()?;
        self.engine.outputs_using(
            &mut super::record_access::StoredAccess {
                evidence: &self.evidence,
                max_bytes: self.limits.checkpoint.bytes,
                store,
            },
            run,
        )
    }

    /// Protected current adopted node outputs, even when other branches are
    /// pending or unrequested. Uses the same resolver as whole-run outputs.
    pub fn read_node_outputs(
        &self,
        run: &str,
        node: &str,
        store: &mut impl RecordStore,
    ) -> Result<Option<Values>> {
        self.check_live()?;
        let store = &mut BudgetedStore::new(store, self.limits.record_reads);
        self.engine.node_outputs_using(
            &mut super::record_access::StoredAccess {
                evidence: &self.evidence,
                max_bytes: self.limits.checkpoint.bytes,
                store,
            },
            run,
            node,
        )
    }

    /// Resolve an actual input binding without activating its consumer. This is
    /// protected source data, suitable for source-owner checks, never diagnostics.
    pub fn read_node_input(
        &self,
        run: &str,
        node: &str,
        port: &str,
        store: &mut impl RecordStore,
    ) -> Result<Option<serde_json::Value>> {
        self.check_live()?;
        let store = &mut BudgetedStore::new(store, self.limits.record_reads);
        self.engine.node_input_using(
            &mut super::record_access::StoredAccess {
                evidence: &self.evidence,
                max_bytes: self.limits.checkpoint.bytes,
                store,
            },
            run,
            node,
            port,
        )
    }

    /// Protected source-content read, not diagnostics. The caller must serialize
    /// workspace/reset authority with capture collection and this synchronous read.
    /// Subscription identity, notifications and transport are owned by the host.
    pub fn read_live_inspection(
        &self,
        run: &str,
        captures: &[EvidenceSnapshot],
        limits: LiveReadLimits,
        store: &mut impl RecordStore,
    ) -> Result<LiveInspection> {
        self.check_live()?;
        let store = &mut BudgetedStore::new(store, self.limits.record_reads);
        self.engine.live_read(
            &mut super::record_access::StoredAccess {
                evidence: &self.evidence,
                max_bytes: self.limits.checkpoint.bytes,
                store,
            },
            self.stamp(),
            run,
            captures,
            limits,
        )
    }
    pub fn poisoned(&self) -> bool {
        self.poisoned
    }

    fn check_live(&self) -> Result<()> {
        if self.poisoned {
            Err(fault(CoreFaultCode::ReloadRequired, "checkpoint"))
        } else {
            Ok(())
        }
    }

    fn commit(
        &mut self,
        candidate: Engine,
        intent: CommitIntent<'_>,
        store: &mut impl CommitStore,
    ) -> Result<()> {
        let next = self
            .checkpoint
            .capture_next(&candidate, self.limits.checkpoint)?;
        self.reserve(&candidate, &next, store)?;
        let records = RecordChanges::between(Some(&self.engine.state), &candidate.state);
        let evidence =
            self.evidence
                .advance(next.stamp(), &records, self.limits.checkpoint.bytes)?;
        match store.commit(CommitRequest {
            expected: Some(self.stamp()),
            next: &next,
            intent,
            records,
        }) {
            Ok(()) => {
                self.engine = candidate;
                self.evidence = evidence;
                self.checkpoint = next;
                Ok(())
            }
            Err(CommitFailure::Rejected(f)) => Err(f),
            Err(CommitFailure::Indeterminate(f)) => {
                self.poisoned = true;
                Err(f)
            }
        }
    }

    /// Preserve the complete current checkpoint as an immutable archive before
    /// replacing its journal with native record commitments at the same revision.
    /// Legacy empty snapshots are repacked against their existing archive parent;
    /// otherwise an empty suffix is a true no-op.
    pub fn compact(&mut self, store: &mut impl CommitStore) -> Result<bool> {
        let store = &mut BudgetedStore::new(store, self.limits.record_reads);
        self.check_live()?;
        let repack = self.engine.journal().is_empty() && self.checkpoint.has_inline_records();
        if self.engine.journal().is_empty() && !repack {
            return Ok(false);
        }
        let usage = if repack {
            self.history_usage
        } else {
            self.history_usage
                .add(&self.checkpoint, self.limits.history)?
        };
        let mut candidate = self.engine.clone();
        candidate.rebase();
        let next = self.checkpoint.compacted_records(
            &candidate,
            &self.evidence,
            self.limits.checkpoint,
        )?;
        self.reserve(&candidate, &next, store)?;
        let evidence = self.evidence.advance(
            next.stamp(),
            &RecordChanges::default(),
            self.limits.checkpoint.bytes,
        )?;
        match store.commit(CommitRequest {
            expected: Some(self.stamp()),
            next: &next,
            intent: if repack {
                CommitIntent::Record
            } else {
                CommitIntent::Compact {
                    archive: &self.checkpoint,
                }
            },
            records: RecordChanges::default(),
        }) {
            Ok(()) => {
                self.engine = candidate;
                self.evidence = evidence;
                self.checkpoint = next;
                self.history_usage = usage;
                Ok(true)
            }
            Err(CommitFailure::Rejected(f)) => Err(f),
            Err(CommitFailure::Indeterminate(f)) => {
                self.poisoned = true;
                Err(f)
            }
        }
    }

    pub fn apply(&mut self, event: Event, store: &mut impl CommitStore) -> Result<Vec<Work>> {
        let store = &mut BudgetedStore::new(store, self.limits.record_reads);
        self.check_live()?;
        if matches!(event, Event::Dispatch { .. } | Event::Adopt { .. }) {
            return Err(fault(CoreFaultCode::EffectRequiresOwner, "event"));
        }
        self.limits.validate_event(&event)?;
        if event
            .evidence_identity()
            .is_some_and(|id| id.engine.as_deref() != Some(self.stamp().engine.as_str()))
        {
            return Err(fault(CoreFaultCode::EvidenceIdentity, "engine"));
        }
        if matches!(event, Event::Recover)
            && !self
                .engine
                .needs_recovery_using(&mut super::record_access::StoredAccess {
                    evidence: &self.evidence,
                    max_bytes: self.limits.checkpoint.bytes,
                    store,
                })?
        {
            return Ok(Vec::new());
        }
        let (candidate, work) = self.engine.candidate(
            event.clone(),
            &mut super::record_access::StoredAccess {
                evidence: &self.evidence,
                max_bytes: self.limits.checkpoint.bytes,
                store,
            },
        )?;
        let intent = match &event {
            Event::Begin {
                run,
                artifact,
                scope,
                inputs,
                ..
            } => CommitIntent::Begin {
                authority: Authority {
                    run,
                    node: None,
                    artifact,
                    scope,
                },
                inputs,
            },
            _ => CommitIntent::Record,
        };
        self.commit(candidate, intent, store)?;
        Ok(work)
    }

    pub fn claim(
        &mut self,
        run: &str,
        node: &str,
        attempt: AttemptId,
        store: &mut impl CommitStore,
    ) -> Result<Invocation> {
        let store = &mut BudgetedStore::new(store, self.limits.record_reads);
        self.check_live()?;
        let mut records = super::record_access::StoredAccess {
            evidence: &self.evidence,
            max_bytes: self.limits.checkpoint.bytes,
            store,
        };
        let (owner, execution) = self
            .engine
            .dispatch_owner(&mut records, run, node, attempt)?;
        let dispatch = self.engine.dispatch(&mut records, execution)?;
        let work = &dispatch.producer.work;
        let mut invocation = self.engine.bind_invocation(work.clone())?;
        invocation.engine = Some(self.stamp().engine.clone());
        let (candidate, _) = self.engine.candidate(
            Event::Dispatch { execution },
            &mut super::record_access::LoadedAccess {
                state: &self.engine.state,
                records: &mut records,
            },
        )?;
        self.commit(
            candidate,
            CommitIntent::Dispatch {
                authority: Authority {
                    run,
                    node: Some(node),
                    artifact: &owner.artifact,
                    scope: &owner.scope,
                },
                work,
                attempt,
            },
            store,
        )?;
        Ok(invocation)
    }

    pub fn adopt(
        &mut self,
        run: &str,
        node: &str,
        attempt: AttemptId,
        store: &mut impl CommitStore,
    ) -> Result<()> {
        let store = &mut BudgetedStore::new(store, self.limits.record_reads);
        self.check_live()?;
        let adoption = self.engine.adoption(
            &mut super::record_access::StoredAccess {
                evidence: &self.evidence,
                max_bytes: self.limits.checkpoint.bytes,
                store,
            },
            run,
            node,
            attempt,
        )?;
        let owner = &adoption.owner;
        let execution = adoption.producer.work.execution;
        let (candidate, _) = self.engine.candidate(
            Event::Adopt {
                run: run.into(),
                node: node.into(),
                attempt,
            },
            &mut super::record_access::LoadedAccess {
                state: &self.engine.state,
                records: &mut super::record_access::StoredAccess {
                    evidence: &self.evidence,
                    max_bytes: self.limits.checkpoint.bytes,
                    store,
                },
            },
        )?;
        self.commit(
            candidate,
            CommitIntent::Adopt {
                authority: Authority {
                    run,
                    node: Some(node),
                    artifact: &owner.artifact,
                    scope: &owner.scope,
                },
                execution,
                attempt,
                values: adoption.values(),
            },
            store,
        )
    }

    fn reserve(
        &self,
        candidate: &Engine,
        checkpoint: &Checkpoint,
        store: &mut impl RecordStore,
    ) -> Result<()> {
        self.limits.reserve_using(
            candidate,
            checkpoint,
            &mut super::record_access::LoadedAccess {
                state: &candidate.state,
                records: &mut super::record_access::StoredAccess {
                    evidence: &self.evidence,
                    max_bytes: self.limits.checkpoint.bytes,
                    store,
                },
            },
        )
    }
}

fn commit_fault(failure: CommitFailure) -> Fault {
    match failure {
        CommitFailure::Rejected(f) | CommitFailure::Indeterminate(f) => f,
    }
}
