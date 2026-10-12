//! Steering and deliberate inspection, independent of learning evidence.
use super::{EffortDimension, award};
use crate::model::Result;
use rusqlite::{Connection, OptionalExtension, params};

pub(crate) fn persona(db: &Connection, request: &crate::ai::generation::Request) -> Result<()> {
    award(
        db,
        EffortDimension::Bot,
        &format!("persona:{}", request.id),
        &request.language_context.language_id,
        &request.language_context.variety_id,
        None,
    )
}

pub(crate) fn steering(
    db: &Connection,
    conversation: &str,
    configuration: &crate::conversations::direction::ConversationStartConfig,
) -> Result<()> {
    use crate::conversations::direction::TopicChoice;
    if configuration.prompt_editor != Some(true)
        && !matches!(
            configuration.direction.topic,
            Some(TopicChoice::Builtin { .. } | TopicChoice::Custom { .. })
        )
    {
        return Ok(());
    }
    let language: String = db.query_row(
        "SELECT language_id FROM conversations WHERE id=?1",
        [conversation],
        |r| r.get(0),
    )?;
    award(
        db,
        EffortDimension::Bot,
        &format!("steering:{conversation}"),
        &language,
        &configuration.variety_id,
        Some(conversation),
    )
}

/// Only called after an explicitly selected attempt's details were displayed.
/// IDs resolve their own attribution; the caller never supplies a language or count.
#[cfg(test)]
pub(crate) fn inspect(
    db: &mut Connection,
    engine: &str,
    run: &str,
    revision: &str,
    node: &str,
    attempt: &str,
) -> Result<Option<super::EffortAward>> {
    let detail = crate::ai::inspection::attempt(db, engine, run, revision, node, attempt)?;
    award_inspection(db, &detail)
}

fn award_inspection(
    db: &mut Connection,
    detail: &crate::ai::inspection::NativeAttemptInspection,
) -> Result<Option<super::EffortAward>> {
    let (engine, run, attempt) = (&detail.engine, &detail.run, &detail.attempt);
    let source = format!("inspection:{}", serde_json::to_string(&(engine, attempt))?);
    if db.query_row(
        "SELECT EXISTS(SELECT 1 FROM effort_awards WHERE dimension='bot' AND source_id=?1)",
        [&source],
        |r| r.get::<_, bool>(0),
    )? {
        return Ok(None);
    }
    if detail.evidence.is_none() && detail.retained_text.is_none() {
        return Ok(None);
    }
    let tx = db.transaction()?;
    let owner: Option<(String,String,String)> = tx.query_row(
        "SELECT t.conversation_id,c.language_id,json_extract(t.context,'$.practiceSettings.varietyId') FROM graph_conversation_runs r JOIN turns t ON t.id=r.turn_id JOIN conversations c ON c.id=t.conversation_id WHERE r.engine_id=?1 AND r.run_id=?2 AND t.conversation_id=?3",
        params![engine,run,detail.owner], |r| Ok((r.get(0)?,r.get(1)?,r.get(2)?))).optional()?;
    let Some((conversation, language, variety)) = owner else {
        return Ok(None);
    };
    award(
        &tx,
        EffortDimension::Bot,
        &source,
        &language,
        &variety,
        Some(&conversation),
    )?;
    let earned = if tx.changes() != 0 {
        let earned = super::read_scoped(&tx, &language, Some(&conversation))?
            .recent
            .into_iter()
            .find(|item| item.source_id == source);
        tx.execute("UPDATE metadata SET revision=revision+1", [])?;
        earned
    } else {
        None
    };
    tx.commit()?;
    Ok(earned)
}

#[tauri::command]
pub(crate) async fn record_bot_inspection(
    state: tauri::State<'_, std::sync::Arc<crate::application::Application>>,
    engine: String,
    run: String,
    revision: String,
    node: String,
    attempt: String,
) -> Result<Option<super::EffortAward>> {
    let state = state.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        let prepared = crate::ai::inspection::prepare(&state.lock()?.connection, &engine, &run)?;
        let key =
            serde_json::to_string(&(prepared.cache_identity()?, &run, &revision, &node, &attempt))?;
        let detail = state.inspect_cached(key, || {
            prepared.attempt(&engine, &run, &revision, &node, &attempt)
        })?;
        award_inspection(&mut state.lock()?.connection, &detail)
    })
    .await
    .map_err(|cause| {
        crate::diagnostics::failures::join(
            &cause,
            "inspection_credit_worker",
            crate::model::AppError::new(
                crate::model::ErrorCode::Internal,
                "Inspection credit worker failed.",
            ),
        )
    })?
}
