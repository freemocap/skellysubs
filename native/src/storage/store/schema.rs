use super::*;

/// Workspace format, including persisted JSON; every bump requires a migration.
/// Bump when required stored fields or their meaning change, not only for SQL DDL.
pub(crate) const SCHEMA_VERSION: i32 = 72;
pub(super) const GENERATION_SCHEMA: &str = include_str!("../schemas/generation_schema.sql");

pub(crate) fn validate_database(connection: &Connection) -> Result<()> {
    let application_id: i32 =
        connection.pragma_query_value(None, "application_id", |r| r.get(0))?;
    if application_id != 1397443659 {
        return Err(AppError::new(
            ErrorCode::Storage,
            "Unexpected database identity.",
        ));
    }
    let integrity: String = connection.query_row("PRAGMA quick_check", [], |r| r.get(0))?;
    if integrity != "ok" {
        return Err(AppError::new(ErrorCode::Storage, integrity));
    }
    if connection.prepare("PRAGMA foreign_key_check")?.exists([])? {
        return Err(AppError::new(
            ErrorCode::Storage,
            "Invalid database ownership references.",
        ));
    }
    Ok(())
}

/// Check current DDL, including revision constraints, before startup writes.
pub(crate) fn validate_current_schema(connection: &Connection) -> Result<()> {
    let reference = Connection::open_in_memory()?;
    for schema in [include_str!("../schemas/schema.sql"), GENERATION_SCHEMA] {
        reference.execute_batch(schema)?;
    }
    create_extensions(&reference)?;
    validate_objects(connection, &reference, false)
}

pub(super) fn create_extensions(connection: &Connection) -> Result<()> {
    for sql in [
        include_str!("../schemas/streaks.sql"),
        include_str!("../schemas/settings.sql"),
        include_str!("../schemas/reading_preloads.sql"),
        include_str!("../schemas/audio_signals.sql"),
        include_str!("../schemas/inference_results.sql"),
        include_str!("../schemas/recording_results.sql"),
        include_str!("../schemas/skill_level_events.sql"),
        include_str!("../schemas/graph_runtime.sql"),
        include_str!("../schemas/workspace_graph_runtime.sql"),
        include_str!("../schemas/turn_execution_owners.sql"),
        include_str!("../schemas/graph_publications.sql"),
        include_str!("../schemas/graph_reply_sources.sql"),
        include_str!("../schemas/graph_assessment_owners.sql"),
        include_str!("../schemas/graph_audio.sql"),
        include_str!("../schemas/graph_audio_delivery.sql"),
        include_str!("../schemas/graph_transport.sql"),
        include_str!("../schemas/graph_speech_requests.sql"),
        include_str!("../schemas/graph_helper_requests.sql"),
    ] {
        connection.execute_batch(sql)?;
    }
    Ok(())
}

pub(super) fn validate_objects(
    connection: &Connection,
    reference: &Connection,
    optional: bool,
) -> Result<()> {
    let mut statement = reference.prepare("SELECT type,name,sql FROM sqlite_master WHERE sql IS NOT NULL AND name NOT LIKE 'sqlite_%'")?;
    for row in statement.query_map([], |r| {
        Ok((
            r.get::<_, String>(0)?,
            r.get::<_, String>(1)?,
            r.get::<_, String>(2)?,
        ))
    })? {
        let (kind, name, sql) = row?;
        let actual: Option<(String, String)> = connection
            .query_row(
                "SELECT type,sql FROM sqlite_master WHERE name=?1",
                [&name],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .optional()?;
        if optional && actual.is_none() {
            continue;
        }
        // Source checkout line endings are not a database format difference.
        if actual.map(|(kind, sql)| (kind, sql.replace("\r\n", "\n")))
            != Some((kind, sql.replace("\r\n", "\n")))
        {
            return Err(AppError::new(
                ErrorCode::Storage,
                format!("Unexpected schema object: {name}. No data was changed."),
            ));
        }
    }
    Ok(())
}
