use super::*;
use crate::model::TurnControl;

impl Runtime {
    pub fn control(&mut self, tx: Transaction<'_>, run: &str, control: TurnControl) -> Result<()> {
        let (conversation, state): (String, String) = tx.query_row(
            "SELECT conversation_id,state FROM turns WHERE id=?1",
            [run],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )?;
        let engine_id = self.owner_engine(&tx, &conversation, run)?;
        let partition = self.partition(&engine_id)?;
        let engine = self.engines.get(&engine_id).ok_or_else(|| {
            AppError::new(ErrorCode::Conflict, "Graph implementation unavailable.")
        })?;
        let event = match control {
            TurnControl::Cancel => {
                if !matches!(state.as_str(), "pending" | "assisting") {
                    return Err(AppError::new(
                        ErrorCode::Conflict,
                        "Only pending turns can be cancelled.",
                    ));
                }
                tx.execute("UPDATE turns SET state='cancelled' WHERE id=?1", [run])?;
                Event::Cancel {
                    run: run.into(),
                    node: None,
                }
            }
            TurnControl::Pause | TurnControl::Resume => {
                if !matches!(state.as_str(), "pending" | "assisting") {
                    return Err(AppError::new(
                        ErrorCode::Conflict,
                        "This turn is not pending.",
                    ));
                }
                let paused = matches!(control, TurnControl::Pause);
                if !paused {
                    super::super::release_hold(&tx, run, false)?;
                }
                tx.execute(
                    "UPDATE turns SET paused=?2 WHERE id=?1",
                    params![run, paused],
                )?;
                Event::Pause {
                    run: run.into(),
                    paused,
                }
            }
            TurnControl::Retry => {
                if !matches!(state.as_str(), "failed" | "unknown") {
                    return Err(AppError::new(
                        ErrorCode::Conflict,
                        "Only failed or unknown turns can be retried.",
                    ));
                }
                let view = engine.inspect(run).map_err(error)?;
                if tx.query_row("SELECT EXISTS(SELECT 1 FROM turns WHERE conversation_id=?1 AND rowid>(SELECT rowid FROM turns WHERE id=?2))",params![conversation,run],|r|r.get::<_,bool>(0))? {
                    return Err(AppError::new(ErrorCode::Conflict,"A later turn exists. Start a new exchange instead of inserting an earlier reply."));
                }
                if view
                    .attempts
                    .iter()
                    .filter(|(node, _)| {
                        view.artifact.operations[&view.artifact.definition.nodes[*node].operation]
                            .resource
                            == Resource::Provider
                    })
                    .map(|(_, attempts)| attempts.len())
                    .sum::<usize>()
                    >= super::super::TURN_ATTEMPT_LIMIT as usize
                {
                    return Err(super::super::budget_error(
                        "This turn reached its attempt limit. Start a new exchange.",
                    ));
                }
                let reply: String = tx.query_row(
                    "SELECT node_key FROM conversation_graph_effects WHERE turn_id=?1",
                    [run],
                    |r| r.get(0),
                )?;
                let primary = view.nodes.get(&reply).ok_or_else(|| {
                    AppError::new(
                        ErrorCode::Storage,
                        "Native reply declaration is missing from its artifact.",
                    )
                })?;
                let candidates = if matches!(primary, Disposition::Failed | Disposition::Unknown) {
                    std::collections::BTreeSet::from([reply])
                } else {
                    super::ancestors(&view, &reply)?
                };
                let node = candidates
                    .into_iter()
                    .find(|node| {
                        matches!(view.nodes[node], Disposition::Failed | Disposition::Unknown)
                    })
                    .ok_or_else(|| {
                        AppError::new(
                            ErrorCode::Conflict,
                            "No failed reply dependency is available to retry.",
                        )
                    })?;
                super::super::admit_network_work(&tx, self, budget::requested(&view, &node)?)?;
                tx.execute(
                    "UPDATE turns SET state='pending',paused=0 WHERE id=?1",
                    [run],
                )?;
                Event::Retry {
                    run: run.into(),
                    node,
                }
            }
            TurnControl::Step => {
                if !matches!(state.as_str(), "pending" | "assisting")
                    || crate::ai::connections::configuration::config(&tx)?.paused
                    || tx.query_row(
                        "SELECT refusal_hold IS NOT NULL OR paused=0 FROM turns WHERE id=?1",
                        [run],
                        |r| r.get::<_, bool>(0),
                    )?
                {
                    return Err(AppError::new(
                        ErrorCode::Conflict,
                        "Step requires a paused turn without an AI pause or refusal hold.",
                    ));
                }
                Event::Step { run: run.into() }
            }
        };
        let callback = |_: &Connection, _: &CommitRequest<'_>| Ok(());
        let mut adapter = TransactionStore::new(tx, partition, limits().checkpoint.bytes, callback);
        self.engines
            .get_mut(&engine_id)
            .unwrap()
            .apply(event, &mut adapter)
            .map_err(error)?;
        Ok(())
    }
}
