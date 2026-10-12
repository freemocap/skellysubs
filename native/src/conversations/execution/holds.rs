use super::*;

pub(super) fn pause_related(
    db: &Connection,
    target: &crate::ai::connections::access::ResolvedTarget,
    error: &AppError,
) -> Result<()> {
    let Some(refusal) = &error.refusal else {
        return Ok(());
    };
    let rows = db
        .prepare(
            "SELECT id,context,refusal_hold FROM turns WHERE state IN ('pending','assisting')",
        )?
        .query_map([], |r| {
            Ok((
                r.get::<_, String>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, Option<String>>(2)?,
            ))
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    for (id, context, existing) in rows {
        let context: serde_json::Value = serde_json::from_str(&context)?;
        let other: crate::ai::connections::access::ResolvedTarget =
            serde_json::from_value(context["target"].clone())?;
        let same = if target.route == ConnectionRoute::Hosted && refusal.service_wide {
            other.route == ConnectionRoute::Hosted
        } else {
            other.route == target.route
                && other.url == target.url
                && other.credential == target.credential
        };
        if same {
            if let Some(existing) = existing {
                let existing: AppError = serde_json::from_str(&existing)?;
                if existing
                    .refusal
                    .and_then(|r| r.retry_at)
                    .is_some_and(|old| old > refusal.retry_at.unwrap_or(0.0))
                {
                    continue;
                }
            }
            db.execute(
                "UPDATE turns SET paused=1,refusal_hold=?2 WHERE id=?1",
                params![id, serde_json::to_string(error)?],
            )?;
        }
    }
    bump(db)
}

pub(super) fn release_hold(db: &Connection, turn: &str, step: bool) -> Result<()> {
    let hold: Option<String> =
        db.query_row("SELECT refusal_hold FROM turns WHERE id=?1", [turn], |r| {
            r.get(0)
        })?;
    if let Some(hold) = hold {
        let error: AppError = serde_json::from_str(&hold)?;
        if step {
            return Err(AppError::new(
                ErrorCode::Provider,
                format!(
                    "Work is held after a provider refusal. Explicitly Resume or Retry to ask the server again. {}",
                    error.message
                ),
            ));
        }
        db.execute(
            "UPDATE turns SET refusal_hold=NULL,paused=0 WHERE id=?1",
            [turn],
        )?;
    }
    Ok(())
}
