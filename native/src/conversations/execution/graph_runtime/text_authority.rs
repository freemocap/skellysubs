//! Revalidate a native producer through a current consumer and its original Work.
use super::super::graph_text_request;
use super::*;
use crate::ai::transport::graph_identity;

impl Runtime {
    pub fn authorize_text(
        &self,
        db: &Connection,
        identity: &InvocationIdentity,
    ) -> Result<graph_text_request::Prepared> {
        let work = self.authorize_work(db, identity)?;
        graph_text_request::prepare(&work, graph_identity::load(db, identity)?)
    }
    pub(super) fn authorize_work(
        &self,
        db: &Connection,
        identity: &InvocationIdentity,
    ) -> Result<Work> {
        let rejected = || {
            AppError::new(
                ErrorCode::Conflict,
                "Native text producer is no longer authorized.",
            )
        };
        let engine_id = identity.engine.as_deref().ok_or_else(rejected)?;
        let engine = self.engines.get(engine_id).ok_or_else(rejected)?;
        let tx = db.unchecked_transaction()?;
        let work = {
            let mut reader = graph_store::BorrowedReadStore::new(&tx, self.partition(engine_id)?)
                .map_err(error)?;
            engine
                .read_execution_work(identity.execution, &mut reader)
                .map_err(error)?
        };
        if work.operation != identity.operation || work.artifact != identity.artifact {
            return Err(rejected());
        }
        let candidates = tx.prepare("SELECT o.run_id,o.scope FROM graph_conversation_runs o JOIN turns t ON t.id=o.turn_id WHERE o.engine_id=?1 AND o.artifact_id=?2 AND t.state NOT IN ('cancelled','invalidated')")?
            .query_map(params![engine_id,identity.artifact], |r|Ok((r.get::<_,String>(0)?,r.get::<_,String>(1)?)))?.collect::<rusqlite::Result<Vec<_>>>()?;
        let mut last_error = None;
        for (run, scope) in candidates {
            let view = engine.inspect(&run).map_err(error)?;
            for (node, attempts) in &view.attempts {
                if !attempts.last().is_some_and(|a| {
                    a.execution == identity.execution && matches!(a.state, AttemptState::Running)
                }) {
                    continue;
                }
                let authority = Authority {
                    run: &run,
                    node: Some(node),
                    artifact: &identity.artifact,
                    scope: &scope,
                };
                let phase = if view.stepping.as_deref() == Some(node) {
                    graph_authority::Phase::SteppedDispatch
                } else {
                    graph_authority::Phase::Dispatch
                };
                let authorized = if work.operation == prose::operation_contract() {
                    graph_authority::check_owner(
                        &tx,
                        engine_id,
                        &authority,
                        &prose::decode(&work.inputs).map_err(error)?,
                        phase,
                    )
                } else {
                    graph_authority::check_helper(&tx, engine_id, &authority, &work, phase)
                };
                match authorized {
                    Ok(()) => {
                        return Ok(work);
                    }
                    Err(cause) if cause.code == ErrorCode::Conflict => last_error = Some(cause),
                    Err(cause) => return Err(cause),
                }
            }
        }
        Err(last_error.unwrap_or_else(rejected))
    }
}
