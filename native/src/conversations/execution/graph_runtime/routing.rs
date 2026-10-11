//! Persisted owner identities select engines; registering a new artifact never
//! changes the catalog of an existing run or retargets its executable.
use super::*;

impl Runtime {
    pub(crate) fn persisted_attempt_is_current(
        &self,
        db: &Connection,
        engine: &str,
        run: &str,
        node: &str,
        attempt: &str,
    ) -> Result<Option<bool>> {
        if let Some(view) = self.retained_view(engine, run) {
            return Ok(Some(
                view.nodes.get(node) == Some(&Disposition::Adopted)
                    && view
                        .attempts
                        .get(node)
                        .and_then(|a| a.last())
                        .is_some_and(|a| a.id == attempt),
            ));
        }
        let Some(host) = self.engines.get(engine) else {
            return Ok(None);
        };
        let mut reader =
            graph_store::BorrowedReadStore::new(db, self.partition(engine)?).map_err(error)?;
        Ok(Some(
            host.read_adopted_attempt(run, node, &mut reader)
                .map_err(error)?
                .map(|id| serde_json::to_string(&id))
                .transpose()?
                .as_deref()
                == Some(attempt),
        ))
    }
    /// Current authoritative persisted state, without replaying historical events.
    /// The caller holds a domain read transaction; every record is stamp-checked.
    pub(crate) fn persisted_inspection(
        &self,
        db: &Connection,
        engine: &str,
        run: &str,
    ) -> Result<Option<InspectionSnapshot>> {
        if let Some(view) = self.retained_view(engine, run) {
            return Ok(Some(view));
        }
        let Some(host) = self.engines.get(engine) else {
            return Ok(None);
        };
        let mut reader =
            graph_store::BorrowedReadStore::new(db, self.partition(engine)?).map_err(error)?;
        host.read_inspection(
            run,
            ExportLimits {
                bytes: 4 * 1024 * 1024,
                attempts: 4096,
            },
            &mut reader,
        )
        .map(Some)
        .map_err(error)
    }

    #[cfg(test)]
    pub fn history(
        &self,
        db: &Connection,
        conversation: &str,
        run: &str,
        before: Option<&str>,
        count: u32,
    ) -> Result<RunHistory> {
        let engine = self.owner_engine(db, conversation, run)?;
        crate::ai::inspection::prepare(db, &engine, run)?.history(run, before, count)
    }

    pub(super) fn retained(
        &self,
        db: &Connection,
        conversation: &str,
        run: &str,
    ) -> Result<HistoricalInspection> {
        let engine = self.owner_engine(db, conversation, run)?;
        let catalog = db.query_row(
            "SELECT catalog FROM graph_engines WHERE id=?1",
            [&engine],
            |r| r.get(0),
        )?;
        let mut reader = graph_store::ReadStore::from_transaction(
            db.unchecked_transaction()?,
            Partition {
                owner: crate::ai::graph_store::Owner::Conversation(conversation.into()),
                catalog,
            },
        );
        let checkpoint = reader
            .checkpoint(limits().checkpoint)
            .map_err(error)?
            .ok_or_else(|| AppError::new(ErrorCode::Storage, "Graph checkpoint missing."))?;
        checkpoint
            .historical_inspection(
                checkpoint.stamp().revision,
                HistoricalLimits {
                    history: limits().history,
                    state: limits().state,
                },
                &mut reader,
            )
            .map_err(error)
    }

    pub(crate) fn register_artifact(&mut self, graph: Arc<Executable>) -> Result<String> {
        let catalog = graph_store::catalog_id([graph.identity()]).map_err(error)?;
        self.catalogs
            .entry(catalog.clone())
            .or_insert_with(|| vec![graph]);
        Ok(catalog)
    }
    pub(super) fn owner_engine(
        &self,
        db: &Connection,
        conversation: &str,
        run: &str,
    ) -> Result<String> {
        db.query_row(
            "SELECT o.engine_id FROM graph_conversation_runs o JOIN turns t ON t.id=o.turn_id JOIN graph_engines e ON e.id=o.engine_id WHERE o.run_id=?1 AND t.conversation_id=?2 AND e.conversation_id=?2",
            params![run, conversation], |r| r.get(0),
        ).map_err(Into::into)
    }

    pub(super) fn partition(&self, engine: &str) -> Result<Partition> {
        self.partitions.get(engine).cloned().ok_or_else(|| {
            AppError::new(
                ErrorCode::Conflict,
                "Graph engine partition is unavailable.",
            )
        })
    }

    pub(super) fn install(&mut self, partition: Partition, engine: DurableEngine) {
        let id = engine.stamp().engine.clone();
        self.partitions.insert(id.clone(), partition);
        self.engines.insert(id, engine);
    }
}
