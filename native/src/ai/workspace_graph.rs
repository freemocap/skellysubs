//! Workspace-owned execution of registered native artifacts. Domain callers own
//! source authority and publication; the core owns all readiness and sharing.
use super::{
    graph::*,
    graph_store::{self, Owner, Partition, TransactionStore},
};
use rusqlite::Connection;
use std::{collections::BTreeMap, sync::Arc};

#[derive(Default)]
pub struct Runtime {
    engines: BTreeMap<String, (Partition, DurableEngine)>,
}
pub enum Progress {
    Waiting,
    Held,
    Invoke(Invocation),
    Complete(Values),
    Failed(Fault),
    Unknown,
    Cancelled,
}

pub fn single_operation(
    registry: Registry,
    contract: Contract,
    operation: Contract,
    inputs: Ports,
    outputs: Ports,
) -> Result<Executable> {
    let node = "operation".to_string();
    let definition = Definition {
        contract,
        nodes: BTreeMap::from([(
            node.clone(),
            Node {
                operation,
                inputs: inputs
                    .keys()
                    .map(|key| (key.clone(), Source::Input(key.clone())))
                    .collect(),
                after: vec![],
                guard: None,
                activation: Activation::Automatic,
            },
        )]),
        results: outputs
            .keys()
            .map(|key| {
                (
                    key.clone(),
                    Source::Output {
                        node: node.clone(),
                        port: key.clone(),
                    },
                )
            })
            .collect(),
        inputs,
        outputs,
        compositions: BTreeMap::new(),
    };
    registry.compile(definition)
}
fn limits() -> DurableLimits {
    DurableLimits {
        record_reads: RecordReadLimits {
            records: 100_000,
            bytes: 512 * 1024 * 1024,
        },
        state: StateLimits {
            runs: 4096,
            attempts: 32768,
            executions: 32768,
        },
        checkpoint: CheckpointLimits {
            bytes: 32 * 1024 * 1024,
            events: 4096,
        },
        settlement_event_bytes: 2 * 1024 * 1024,
        history: HistoryLimits {
            bytes: 512 * 1024 * 1024,
            events: 100_000,
            segments: 4096,
        },
    }
}
fn fault() -> Fault {
    Fault {
        code: "workspace_graph_unavailable".into(),
        path: "owner".into(),
    }
}
fn storage(error: rusqlite::Error) -> Fault {
    graph_store::sql(error, "workspace_transaction")
}

pub fn error(fault: Fault) -> crate::model::AppError {
    crate::model::AppError::new(
        if fault.code.starts_with("graph_storage_") {
            crate::model::ErrorCode::Storage
        } else {
            crate::model::ErrorCode::Conflict
        },
        "Native workspace execution could not complete this transition.",
    )
    .with_diagnostics(
        serde_json::json!({"stage":"workspace_graph","code":fault.code,"path":fault.path}),
    )
}
impl Runtime {
    pub(crate) fn inspect_run(
        &self,
        db: &Connection,
        engine: &str,
        run: &str,
    ) -> crate::model::Result<Option<InspectionSnapshot>> {
        let Some((partition, host)) = self
            .engines
            .values()
            .find(|(_, host)| host.stamp().engine == engine)
        else {
            return Ok(None);
        };
        let tx = db.unchecked_transaction()?;
        let mut reader =
            graph_store::BorrowedReadStore::new(&tx, partition.clone()).map_err(error)?;
        host.read_inspection(
            run,
            ExportLimits {
                bytes: 8 * 1024 * 1024,
                attempts: 4096,
            },
            &mut reader,
        )
        .map(Some)
        .map_err(error)
    }
    /// End a failed request's consumer ownership without retrying its producer.
    pub fn cancel_run(&mut self, db: &mut Connection, run: &str) -> Result<()> {
        use rusqlite::OptionalExtension;
        let catalog: Option<String> = db.query_row("SELECT e.catalog FROM workspace_graph_runs r JOIN workspace_graph_engines e ON e.id=r.engine_id WHERE r.run_id=?1", [run], |r| r.get(0)).optional().map_err(storage)?;
        if let Some(catalog) = catalog {
            self.event(
                db,
                &catalog,
                Event::Cancel {
                    run: run.into(),
                    node: None,
                },
                |_, _| Ok(()),
            )?;
        }
        Ok(())
    }

    /// Recover without retrying interrupted work, then admit a new explicit run.
    /// The callback shares the native transaction for every product side effect.
    #[allow(clippy::too_many_arguments)]
    pub fn begin(
        &mut self,
        db: &mut Connection,
        workspace: &str,
        graph: Arc<Executable>,
        run: &str,
        inputs: Values,
        reuse_scope: &str,
        kind: &str,
        context: &serde_json::Value,
        mut owner: impl FnMut(&Connection, &CommitRequest<'_>) -> Result<()>,
    ) -> Result<String> {
        let catalog = graph_store::catalog_id([graph.identity()])?;
        if !self.engines.contains_key(&catalog) {
            let partition = Partition {
                owner: Owner::Workspace(workspace.into()),
                catalog: catalog.clone(),
            };
            let checkpoint = graph_store::ReadStore::new(db, partition.clone())?
                .checkpoint(limits().checkpoint)?;
            if let Some(checkpoint) = checkpoint {
                let mut adapter = TransactionStore::new(
                    db.transaction().map_err(storage)?,
                    partition.clone(),
                    limits().checkpoint.bytes,
                    |_: &Connection, _: &CommitRequest<'_>| Ok(()),
                );
                let engine =
                    DurableEngine::recover(checkpoint, [graph.clone()], limits(), &mut adapter)?;
                self.engines.insert(catalog.clone(), (partition, engine));
            }
        }
        let mut publish = |db: &Connection, request: &CommitRequest<'_>| {
            if let CommitIntent::Begin { authority, .. } = &request.intent {
                db.execute("INSERT INTO workspace_graph_runs(run_id,engine_id,artifact_id,kind,context) VALUES(?1,?2,?3,?4,?5)",rusqlite::params![authority.run,request.next.stamp().engine,authority.artifact,kind,context.to_string()]).map_err(storage)?;
            }
            owner(db, request)
        };
        let event = Event::Begin {
            run: run.into(),
            artifact: graph.identity().into(),
            inputs,
            scope: format!("{workspace}:{reuse_scope}"),
            policy: BTreeMap::new(),
        };
        if let Some((partition, engine)) = self.engines.get_mut(&catalog) {
            let mut adapter = TransactionStore::new(
                db.transaction().map_err(storage)?,
                partition.clone(),
                limits().checkpoint.bytes,
                &mut publish,
            );
            engine.apply(event, &mut adapter)?;
        } else {
            let partition = Partition {
                owner: Owner::Workspace(workspace.into()),
                catalog: catalog.clone(),
            };
            let mut adapter = TransactionStore::new(
                db.transaction().map_err(storage)?,
                partition.clone(),
                limits().checkpoint.bytes,
                &mut publish,
            );
            let engine = DurableEngine::create_with_run([graph], limits(), event, &mut adapter)?;
            self.engines.insert(catalog.clone(), (partition, engine));
        }
        Ok(catalog)
    }
    pub fn work(
        &self,
        db: &Connection,
        identity: &InvocationIdentity,
        mut authorized: impl FnMut(&str) -> bool,
    ) -> Result<Work> {
        let (partition, engine) = self
            .engines
            .values()
            .find(|(_, engine)| Some(engine.stamp().engine.as_str()) == identity.engine.as_deref())
            .ok_or_else(fault)?;
        let runs = db
            .prepare("SELECT run_id FROM workspace_graph_runs WHERE engine_id=?1")
            .map_err(storage)?
            .query_map([&engine.stamp().engine], |r| r.get::<_, String>(0))
            .map_err(storage)?
            .collect::<std::result::Result<Vec<_>, _>>()
            .map_err(storage)?;
        let mut active = false;
        for run in runs {
            if authorized(&run)
                && engine.inspect(&run)?.attempts.values().any(|attempts| {
                    attempts.last().is_some_and(|attempt| {
                        attempt.execution == identity.execution
                            && attempt.state == AttemptState::Running
                    })
                })
            {
                active = true;
                break;
            }
        }
        if !active {
            return Err(fault());
        }
        let tx = db.unchecked_transaction().map_err(storage)?;
        let mut reader = graph_store::BorrowedReadStore::new(&tx, partition.clone())?;
        let work = engine.read_execution_work(identity.execution, &mut reader)?;
        if work.artifact != identity.artifact || work.operation != identity.operation {
            return Err(fault());
        }
        Ok(work)
    }
    pub fn result_record(
        &self,
        db: &Connection,
        catalog: &str,
        run: &str,
        node: &str,
    ) -> Result<(String, Attempt, Option<ExecutionEvidence>)> {
        let (_, engine) = self.engines.get(catalog).ok_or_else(fault)?;
        let attempt = (*engine
            .inspect(run)?
            .attempts
            .get(node)
            .and_then(|a| a.last())
            .ok_or_else(fault)?)
        .clone();
        let evidence = self.evidence(db, catalog, attempt.execution)?;
        Ok((engine.stamp().engine.clone(), attempt, evidence))
    }
    pub fn evidence(
        &self,
        db: &Connection,
        catalog: &str,
        execution: ExecutionId,
    ) -> Result<Option<ExecutionEvidence>> {
        let (partition, engine) = self.engines.get(catalog).ok_or_else(fault)?;
        let tx = db.unchecked_transaction().map_err(storage)?;
        let mut reader = graph_store::BorrowedReadStore::new(&tx, partition.clone())?;
        engine.read_execution_evidence(execution, &mut reader)
    }
    pub fn event(
        &mut self,
        db: &mut Connection,
        catalog: &str,
        event: Event,
        owner: impl FnMut(&Connection, &CommitRequest<'_>) -> Result<()>,
    ) -> Result<()> {
        let (partition, engine) = self.engines.get_mut(catalog).ok_or_else(fault)?;
        let mut adapter = TransactionStore::new(
            db.transaction().map_err(storage)?,
            partition.clone(),
            limits().checkpoint.bytes,
            owner,
        );
        engine.apply(event, &mut adapter)?;
        Ok(())
    }
    pub fn poll(
        &mut self,
        db: &mut Connection,
        catalog: &str,
        run: &str,
        capacity: Capacity,
        mut owner: impl FnMut(&Connection, &CommitRequest<'_>) -> Result<()>,
    ) -> Result<Progress> {
        let (partition, engine) = self.engines.get_mut(catalog).ok_or_else(fault)?;
        let (bytes, events) = engine.checkpoint_usage();
        if bytes > limits().checkpoint.bytes / 4 || events > limits().checkpoint.events / 4 {
            let mut adapter = TransactionStore::new(
                db.transaction().map_err(storage)?,
                partition.clone(),
                limits().checkpoint.bytes,
                |_: &Connection, _: &CommitRequest<'_>| Ok(()),
            );
            engine.compact(&mut adapter)?;
        }
        let view = engine.inspect(run)?;
        {
            let mut reader = graph_store::ReadStore::new(db, partition.clone())?;
            if let Some(values) = engine.read_outputs(run, &mut reader)? {
                return Ok(Progress::Complete(values));
            }
        }
        if view
            .nodes
            .values()
            .any(|state| *state == Disposition::Unknown)
        {
            return Ok(Progress::Unknown);
        }
        if view
            .nodes
            .values()
            .any(|state| *state == Disposition::Cancelled)
        {
            return Ok(Progress::Cancelled);
        }
        if let Some(cause) = view
            .attempts
            .values()
            .filter_map(|attempts| attempts.last())
            .find_map(|a| {
                if let AttemptState::Failed(cause) = &a.state {
                    Some(cause.clone())
                } else {
                    None
                }
            })
        {
            return Ok(Progress::Failed(cause));
        }
        let available = view.attempts.iter().find_map(|(node, attempts)| {
            attempts
                .last()
                .filter(|a| a.state == AttemptState::Available)
                .map(|a| (node.clone(), a.id))
        });
        if let Some((node, attempt)) = available {
            let mut adapter = TransactionStore::new(
                db.transaction().map_err(storage)?,
                partition.clone(),
                limits().checkpoint.bytes,
                &mut owner,
            );
            engine.adopt(run, &node, attempt, &mut adapter)?;
            return Ok(Progress::Waiting);
        }
        if view
            .nodes
            .values()
            .any(|state| matches!(state, Disposition::Ready | Disposition::Held))
        {
            let mut adapter = TransactionStore::new(
                db.transaction().map_err(storage)?,
                partition.clone(),
                limits().checkpoint.bytes,
                &mut owner,
            );
            engine.apply(Event::Advance(capacity), &mut adapter)?;
        }
        let view = engine.inspect(run)?;
        if capacity.provider == 0 && view.nodes.values().any(|state| *state == Disposition::Held) {
            return Ok(Progress::Held);
        }
        let prepared = view.attempts.iter().find_map(|(node, attempts)| {
            attempts
                .last()
                .filter(|a| a.state == AttemptState::Prepared)
                .map(|a| (node.clone(), a.id))
        });
        if let Some((node, attempt)) = prepared {
            let mut adapter = TransactionStore::new(
                db.transaction().map_err(storage)?,
                partition.clone(),
                limits().checkpoint.bytes,
                owner,
            );
            return engine
                .claim(run, &node, attempt, &mut adapter)
                .map(Progress::Invoke);
        }
        Ok(Progress::Waiting)
    }
}

pub(crate) mod history;
#[cfg(any(test, feature = "workflow-capture"))]
pub(crate) use history::inspection;
pub use history::{execution_receipt, receipt, usage, usage_for_kind};
