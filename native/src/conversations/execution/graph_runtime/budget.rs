//! Admission counts accepted provider work from validated native facts, including
//! paused and dependency-waiting work. No operation-name budget catalog is kept.
use super::*;

pub(super) fn initial(artifact: &Artifact, policy: &BTreeMap<String, Activation>) -> Result<i64> {
    let mut count = 0;
    for (node, definition) in &artifact.definition.nodes {
        if artifact.operations[&definition.operation].resource == Resource::Provider
            && definition_ancestors(&artifact.definition, node)?
                .iter()
                .all(|parent| {
                    policy
                        .get(parent)
                        .copied()
                        .unwrap_or(artifact.definition.nodes[parent].activation)
                        == Activation::Automatic
                })
        {
            count += 1;
        }
    }
    Ok(count)
}

pub(super) fn requested(view: &Inspection<'_>, target: &str) -> Result<i64> {
    let mut count = 0;
    for (node, definition) in &view.artifact.definition.nodes {
        if view.artifact.operations[&definition.operation].resource != Resource::Provider
            || matches!(
                view.nodes[node],
                Disposition::Adopted
                    | Disposition::Available
                    | Disposition::Running
                    | Disposition::Prepared
            )
        {
            continue;
        }
        let ancestors = definition_ancestors(&view.artifact.definition, node)?;
        if ancestors.contains(target)
            && !ancestors.iter().any(|parent| {
                parent != target
                    && matches!(
                        view.nodes[parent],
                        Disposition::Unrequested
                            | Disposition::Disabled
                            | Disposition::Cancelled
                            | Disposition::Failed
                            | Disposition::Unknown
                            | Disposition::Skipped
                    )
            })
        {
            count += 1;
        }
    }
    Ok(count)
}

pub(in crate::conversations::execution) fn outstanding(
    db: &Connection,
    runtime: &Runtime,
) -> Result<i64> {
    // Borrow the command transaction when present; otherwise hold one read snapshot.
    let _snapshot = db
        .is_autocommit()
        .then(|| db.unchecked_transaction())
        .transpose()?;
    let rows = db
        .prepare(
            "SELECT e.conversation_id,e.catalog,o.run_id,e.id,e.stamp FROM graph_conversation_runs o
         JOIN turns t ON t.id=o.turn_id JOIN graph_engines e ON e.id=o.engine_id
         JOIN conversations c ON c.id=t.conversation_id JOIN contacts p ON p.id=c.contact_id
         WHERE (o.channel='speech' OR t.state IN ('pending','assisting','failed','unknown')) AND t.state NOT IN ('cancelled','invalidated')
         AND c.archived=0 AND p.archived=0 ORDER BY e.id,o.run_id",
        )?
        .query_map([], |r| {
            Ok((
                r.get::<_, String>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, String>(2)?,
                r.get::<_, String>(3)?,
                r.get::<_, String>(4)?,
            ))
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    let mut partitions: BTreeMap<(String, String, String, String), Vec<String>> = BTreeMap::new();
    for (conversation, catalog, run, engine, stamp) in rows {
        partitions
            .entry((conversation, catalog, engine, stamp))
            .or_default()
            .push(run);
    }
    let mut count = 0;
    for ((conversation, catalog, engine_id, stamp), runs) in partitions {
        if let Some(engine) = runtime.engines.get(&engine_id) {
            let persisted: Stamp = serde_json::from_str(&stamp)?;
            if engine.stamp() != &persisted {
                return Err(AppError::new(
                    ErrorCode::Storage,
                    "Admission engine revision differs from storage.",
                ));
            }
            for run in runs {
                if engine.run_control(&run).map_err(error)?.active {
                    let view = engine.inspect(&run).map_err(error)?;
                    count += pending_count(view.artifact, &view.nodes)?;
                }
            }
            continue;
        }
        let mut reader = graph_store::BorrowedReadStore::new(
            db,
            Partition {
                owner: crate::ai::graph_store::Owner::Conversation(conversation),
                catalog,
            },
        )
        .map_err(error)?;
        let checkpoint = reader
            .checkpoint(limits().checkpoint)
            .map_err(error)?
            .ok_or_else(|| {
                AppError::new(
                    ErrorCode::Storage,
                    "Native admission checkpoint is missing.",
                )
            })?;
        let retained = checkpoint
            .historical_inspection(
                checkpoint.stamp().revision,
                HistoricalLimits {
                    history: limits().history,
                    state: limits().state,
                },
                &mut reader,
            )
            .map_err(error)?;
        for run in runs {
            let view = retained
                .snapshot(
                    &run,
                    ExportLimits {
                        bytes: 4 * 1024 * 1024,
                        attempts: 4096,
                    },
                )
                .map_err(error)?;
            if !view.active {
                continue;
            }
            count += pending_count(&view.artifact, &view.nodes)?;
        }
    }
    Ok(count)
}

fn pending_count<C>(artifact: &Artifact<C>, nodes: &BTreeMap<String, Disposition>) -> Result<i64> {
    let mut count = 0;
    for (node, state) in nodes {
        if artifact.operations[&artifact.definition.nodes[node].operation].resource
            != Resource::Provider
        {
            continue;
        }
        let pending = match state {
            Disposition::Ready
            | Disposition::Paused
            | Disposition::Held
            | Disposition::Prepared
            | Disposition::Running => true,
            Disposition::Waiting => !definition_ancestors(&artifact.definition, node)?
                .iter()
                .any(|parent| {
                    matches!(
                        nodes[parent],
                        Disposition::Unrequested
                            | Disposition::Disabled
                            | Disposition::Skipped
                            | Disposition::Cancelled
                            | Disposition::Failed
                            | Disposition::Unknown
                            | Disposition::Blocked
                    )
                }),
            _ => false,
        };
        count += i64::from(pending);
    }
    Ok(count)
}
