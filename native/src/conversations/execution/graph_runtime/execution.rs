use super::*;
use crate::ai::transport::graph_identity;

pub struct Claim {
    pub conversation: String,
    pub run: String,
    pub resource: Resource,
    pub invocation: Invocation,
}

fn owner(
    db: &Connection,
    request: &CommitRequest<'_>,
    work: Option<&Work>,
    learning: Option<(&crate::configuration::Registry, &str)>,
    terminal: Option<(&str, bool)>,
    stepping: bool,
) -> Result<()> {
    match &request.intent {
        CommitIntent::Dispatch { work, .. } if work.resource == Resource::Provider => {
            if stepping {
                graph_authority::dispatch_with_step(db, request, true)?;
            } else {
                graph_authority::dispatch(db, request)?;
            }
            graph_identity::bind(db, request)?;
        }
        CommitIntent::Adopt { authority, .. }
            if work.is_some_and(|w| w.operation == prose::operation_contract()) =>
        {
            let work = work.ok_or_else(|| {
                AppError::new(
                    ErrorCode::Internal,
                    "Missing original graph producer input.",
                )
            })?;
            let captured = prose::decode(&work.inputs).map_err(error)?;
            graph_publication::publish_reply(db, request, |db, authority| {
                graph_authority::check_owner(
                    db,
                    &request.next.stamp().engine,
                    authority,
                    &captured,
                    graph_authority::Phase::Adoption,
                )
            })?;
            db.execute(
                "UPDATE turns SET state='succeeded' WHERE id=?1 AND EXISTS(SELECT 1 FROM turn_execution_owners WHERE turn_id=?1 AND channel='coach')",
                [authority.run],
            )?;
        }
        CommitIntent::Adopt { .. } => {
            let work = work.ok_or_else(|| {
                AppError::new(ErrorCode::Internal, "Missing native adoption producer.")
            })?;
            let authorize = |db: &Connection, authority: &Authority<'_>| {
                graph_authority::check_helper(
                    db,
                    &request.next.stamp().engine,
                    authority,
                    work,
                    graph_authority::Phase::Adoption,
                )
            };
            if !graph_publication::adopt_audio(db, request, work)?
                && !graph_publication::publish_helper(db, request, work, authorize)?
                && (work.operation
                    == crate::learning::coaching::assessment_graph::operation_contract()
                    || work.operation
                        == crate::learning::coaching::feedback_graph::operation_contract())
            {
                let (registry, session) = learning.ok_or_else(|| {
                    AppError::new(
                        ErrorCode::Internal,
                        "Native learning adoption requires its publication context.",
                    )
                })?;
                graph_publication::publish_assessment(
                    db, request, work, registry, session, authorize,
                )?;
            }
        }
        _ => (),
    }
    if let Some((run, true)) = terminal {
        // Domain status is a projection. Preserve cancelled/revoked owners.
        db.execute("UPDATE turns SET state='failed' WHERE state IN ('pending','assisting') AND id IN (SELECT turn_id FROM turn_execution_owners WHERE engine_id=?1 AND run_id=?2 AND channel='coach')",
                params![request.next.stamp().engine, run])?;
    }
    super::super::bump(db)?;
    Ok(())
}

impl Runtime {
    pub fn observe(&mut self, db: &mut Connection, snapshot: EvidenceSnapshot) -> Result<()> {
        let engine_id = self
            .engines
            .iter()
            .find(|(_, engine)| {
                Some(engine.stamp().engine.as_str()) == snapshot.identity.engine.as_deref()
            })
            .map(|(id, _)| id.clone());
        if let Some(engine_id) = engine_id {
            self.transition(db, &engine_id, Event::Observe(snapshot))?;
        }
        Ok(())
    }
    pub fn transition(
        &mut self,
        db: &mut Connection,
        engine_id: &str,
        event: Event,
    ) -> Result<Vec<Work>> {
        self.transition_terminal(db, engine_id, event, None, None)
    }
    fn transition_terminal(
        &mut self,
        db: &mut Connection,
        engine_id: &str,
        event: Event,
        terminal: Option<(&str, bool)>,
        learning: Option<(&crate::configuration::Registry, &str)>,
    ) -> Result<Vec<Work>> {
        self.maintain(db, engine_id)?;
        let partition = self.partition(engine_id)?;
        let engine = self.engines.get_mut(engine_id).ok_or_else(|| {
            AppError::new(
                ErrorCode::Conflict,
                "Graph implementation is unavailable for this conversation.",
            )
        })?;
        let work = if let Event::Adopt { run, node, attempt } = &event {
            let execution = engine.inspect(run).map_err(error)?.attempts[node]
                .iter()
                .find(|a| a.id == *attempt)
                .ok_or_else(|| AppError::new(ErrorCode::Conflict, "Graph attempt is unavailable."))?
                .execution;
            let mut reader = graph_store::ReadStore::new(db, partition.clone()).map_err(error)?;
            Some(
                engine
                    .read_execution_work(execution, &mut reader)
                    .map_err(error)?,
            )
        } else {
            None
        };
        let settlement = match &event {
            Event::SettleObserved(report)
                if report.identity.operation
                    == crate::speech::synthesis_graph::operation_contract() =>
            {
                let pending = self
                    .audio
                    .iter()
                    .find(|(_, audio)| audio.identity == report.identity);
                if report.outcome.is_ok() && pending.is_none() {
                    return Err(AppError::new(
                        ErrorCode::Storage,
                        "Native speech settlement has no producer payload.",
                    ));
                }
                Some((pending, report.outcome.as_ref().ok().cloned()))
            }
            _ => None,
        };
        let deliver_audio = settlement
            .as_ref()
            .is_some_and(|(_, values)| values.is_some());
        let settled_audio = settlement
            .as_ref()
            .and_then(|(pending, _)| pending.map(|(key, _)| key.clone()));
        let mut domain_error = None;
        let callback = |db: &Connection, request: &CommitRequest<'_>| {
            (|| {
                if let Some((Some((_, audio)), Some(values))) = &settlement {
                    audio.retain(db, values)?;
                }
                owner(db, request, work.as_ref(), learning, terminal, false)
            })()
            .map_err(|cause| {
                domain_error = Some(cause);
                Fault {
                    code: "graph_owner_rejected".into(),
                    path: "owner".into(),
                }
            })
        };
        let mut adapter = TransactionStore::new(
            db.transaction()?,
            partition,
            limits().checkpoint.bytes,
            callback,
        );
        let result = match event {
            Event::Adopt { run, node, attempt } => engine
                .adopt(&run, &node, attempt, &mut adapter)
                .map(|_| Vec::new()),
            event => engine.apply(event, &mut adapter),
        };
        drop(adapter);
        let result = result.map_err(|fault| domain_error.unwrap_or_else(|| error(fault)))?;
        if let Some(key) = settled_audio
            && let Some(audio) = self.audio.remove(&key)
        {
            // Delivery is bounded and evictable; the committed receipt survives it.
            if deliver_audio {
                self.audio_delivery
                    .insert(crate::speech::delivery::ReadyAudio {
                        operation_id: key.clone(),
                        attempt_id: key,
                        message_id: audio.request.source.id,
                        wav: audio.wav,
                        alignment: audio.alignment,
                    })?;
            }
        }
        Ok(result)
    }

    pub fn next(
        &mut self,
        db: &mut Connection,
        provider: bool,
        registry: &crate::configuration::Registry,
        session: &str,
    ) -> Result<Option<Claim>> {
        let global_pause = crate::ai::connections::configuration::config(db)?.paused;
        // Terminal coach runs have no automatic work. Keep cancelled/invalidated
        // runs eligible for core cancellation and retain every independent helper.
        let rows=db.prepare("SELECT o.run_id,t.conversation_id,t.state,CASE WHEN o.channel IN ('speech','helper') THEN 0 ELSE t.paused END,t.refusal_hold IS NOT NULL FROM turns t JOIN graph_conversation_runs o ON o.turn_id=t.id WHERE o.channel<>'coach' OR t.state IN ('pending','assisting','cancelled','invalidated') OR EXISTS(SELECT 1 FROM graph_speech_requests WHERE run_id=o.run_id UNION ALL SELECT 1 FROM graph_helper_requests WHERE run_id=o.run_id) ORDER BY t.rowid")?
            .query_map([],|r|Ok((r.get::<_,String>(0)?,r.get::<_,String>(1)?,r.get::<_,String>(2)?,r.get::<_,bool>(3)?,r.get::<_,bool>(4)?)))?.collect::<rusqlite::Result<Vec<_>>>()?;
        let mut pending: std::collections::VecDeque<_> = rows.into();
        while let Some((run, conversation, state, paused, held)) = pending.pop_front() {
            let engine_id = self.owner_engine(db, &conversation, &run)?;
            let Some(engine) = self.engines.get(&engine_id) else {
                continue;
            };
            let control = engine.run_control(&run).map_err(error)?;
            if matches!(state.as_str(), "cancelled" | "invalidated") {
                if control.active {
                    self.transition(
                        db,
                        &engine_id,
                        Event::Cancel {
                            run: run.clone(),
                            node: None,
                        },
                    )?;
                }
                continue;
            }
            if !control.active {
                continue;
            }
            let partner = status::partner(db, engine, &run)?;
            let auxiliary: bool = db.query_row(
                "SELECT EXISTS(SELECT 1 FROM graph_speech_requests WHERE run_id=?1 UNION ALL SELECT 1 FROM graph_helper_requests WHERE run_id=?1)",
                [&run],
                |r| r.get(0),
            )?;
            if !partner && !auxiliary && !matches!(state.as_str(), "pending" | "assisting") {
                continue;
            }
            let pause = paused || held || global_pause;
            if control.paused != pause {
                self.transition(
                    db,
                    &engine_id,
                    Event::Pause {
                        run: run.clone(),
                        paused: pause,
                    },
                )?;
            }
            if held || global_pause {
                continue;
            }
            let view = self.engines[&engine_id].inspect(&run).map_err(error)?;
            if pause && view.stepping.is_none() {
                continue;
            }
            if let Some((node, attempt)) = view.attempts.iter().find_map(|(node, attempts)| {
                attempts
                    .last()
                    .filter(|a| {
                        matches!(a.state, AttemptState::Available)
                            && (!pause || view.stepping.as_ref() == Some(node))
                    })
                    .map(|a| (node.clone(), a.id))
            }) {
                self.transition_terminal(
                    db,
                    &engine_id,
                    Event::Adopt {
                        run: run.clone(),
                        node,
                        attempt,
                    },
                    None,
                    Some((registry, session)),
                )?;
                // Adoption can immediately unlock another node. Reinspect this run
                // in the same pass instead of imposing another scheduler interval.
                pending.push_front((run, conversation, state, paused, held));
                continue;
            }
            if view.nodes.iter().any(|(node, state)| {
                matches!(
                    state,
                    Disposition::Ready | Disposition::Held | Disposition::Prepared
                ) && (!pause || view.stepping.as_ref() == Some(node))
                    && (provider
                        || view.artifact.operations
                            [&view.artifact.definition.nodes[node].operation]
                            .resource
                            == Resource::Local)
            }) {
                self.transition(
                    db,
                    &engine_id,
                    Event::Advance(Capacity {
                        // Capacity is the total concurrent ceiling, not the number of
                        // permits acquired for this single claim. The application still
                        // owns one network permit for every dispatched provider.
                        local: usize::MAX,
                        provider: if provider {
                            crate::ai::policy::admission::NETWORK_CAPACITY
                        } else {
                            0
                        },
                    }),
                )?;
            }
            let view = self.engines[&engine_id].inspect(&run).map_err(error)?;
            let prepared = view.attempts.iter().find_map(|(node, attempts)| {
                attempts
                    .last()
                    .filter(|a| {
                        matches!(a.state, AttemptState::Prepared)
                            && (!pause || view.stepping.as_ref() == Some(node))
                            && (provider
                                || view.artifact.operations
                                    [&view.artifact.definition.nodes[node].operation]
                                    .resource
                                    == Resource::Local)
                    })
                    .map(|a| (node.clone(), a.id))
            });
            let Some((node, attempt)) = prepared else {
                continue;
            };
            let resource =
                view.artifact.operations[&view.artifact.definition.nodes[&node].operation].resource;
            if resource == Resource::Provider && !provider {
                continue;
            }
            let partition = self.partition(&engine_id)?;
            let stepping = view.stepping.as_deref() == Some(&node);
            let mut domain_error = None;
            let callback = |db: &Connection, request: &CommitRequest<'_>| {
                owner(db, request, None, None, None, stepping).map_err(|cause| {
                    domain_error = Some(cause);
                    Fault {
                        code: "graph_dispatch_rejected".into(),
                        path: "owner".into(),
                    }
                })
            };
            let mut adapter = TransactionStore::new(
                db.transaction()?,
                partition,
                limits().checkpoint.bytes,
                callback,
            );
            let result =
                self.engines
                    .get_mut(&engine_id)
                    .unwrap()
                    .claim(&run, &node, attempt, &mut adapter);
            drop(adapter);
            let invocation =
                result.map_err(|fault| domain_error.unwrap_or_else(|| error(fault)))?;
            return Ok(Some(Claim {
                conversation,
                run,
                resource,
                invocation,
            }));
        }
        Ok(None)
    }

    fn maintain(&mut self, db: &mut Connection, engine_id: &str) -> Result<()> {
        let partition = self.partition(engine_id)?;
        let Some(engine) = self.engines.get_mut(engine_id) else {
            return Ok(());
        };
        let (bytes, events) = engine.checkpoint_usage();
        if bytes < limits().checkpoint.bytes / 4 && events < limits().checkpoint.events / 4 {
            return Ok(());
        }
        let mut adapter = TransactionStore::new(
            db.transaction()?,
            partition,
            limits().checkpoint.bytes,
            |_: &Connection, _: &CommitRequest<'_>| Ok(()),
        );
        engine.compact(&mut adapter).map_err(error)?;
        Ok(())
    }

    pub fn finish(
        &mut self,
        db: &mut Connection,
        conversation: &str,
        run: &str,
        mut report: InvocationReport,
    ) -> Result<()> {
        let engine_id = report.identity.engine.clone().ok_or_else(|| {
            AppError::new(
                ErrorCode::Conflict,
                "Native response has no engine identity.",
            )
        })?;
        let present: bool = db.query_row(
            "SELECT EXISTS(SELECT 1 FROM graph_engines WHERE id=?1 AND conversation_id=?2)",
            params![engine_id, conversation],
            |r| r.get(0),
        )?;
        if !present {
            // Whole-conversation deletion removes its execution ownership too.
            return Ok(());
        }
        let original_owner: bool = db.query_row("SELECT EXISTS(SELECT 1 FROM turn_execution_owners WHERE executor='graph' AND engine_id=?1 AND run_id=?2)",params![engine_id, run],|r|r.get(0))?;
        if original_owner && report.identity.operation == prose::operation_contract() {
            self.maintain(db, &engine_id)?;
            let engine = self.engines.get(&engine_id).ok_or_else(|| {
                AppError::new(
                    ErrorCode::Conflict,
                    "Graph implementation is unavailable for this conversation.",
                )
            })?;
            let view = engine.inspect(run).map_err(error)?;
            if view
                .attempts
                .values()
                .flatten()
                .any(|attempt| attempt.execution == report.identity.execution)
            {
                super::reply_validation::validate(db, run, &mut report)?;
            }
        }
        let failed = report.outcome.is_err();
        self.transition_terminal(
            db,
            &engine_id,
            Event::SettleObserved(report),
            original_owner.then_some((run, failed)),
            None,
        )?;
        if !original_owner {
            return Ok(());
        }
        if status::partner(db, &self.engines[&engine_id], run)? {
            return Ok(());
        }
        let view = self.engines[&engine_id].inspect(run).map_err(error)?;
        let failed = view
            .nodes
            .values()
            .any(|s| matches!(s, Disposition::Failed));
        if failed {
            db.execute(
                "UPDATE turns SET state='failed' WHERE id=?1 AND state IN ('pending','assisting')",
                [run],
            )?;
        }
        Ok(())
    }
}
