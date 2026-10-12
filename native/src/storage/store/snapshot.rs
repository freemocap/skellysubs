use super::*;

impl Store {
    /// Read the current workspace owner without projecting unrelated UI data.
    pub(crate) fn learner_id(&self) -> Result<String> {
        Ok(self
            .connection
            .query_row("SELECT id FROM learner WHERE singleton=1", [], |row| {
                row.get(0)
            })?)
    }

    pub fn snapshot(&self) -> Result<Snapshot> {
        read_snapshot(&self.connection, &self.session_id, &self.config)
    }
}

fn decode<T: serde::de::DeserializeOwned>(
    row: &rusqlite::Row<'_>,
    index: usize,
) -> rusqlite::Result<T> {
    let raw: String = row.get(index)?;
    serde_json::from_str(&raw).map_err(|e| {
        rusqlite::Error::FromSqlConversionFailure(index, rusqlite::types::Type::Text, Box::new(e))
    })
}

pub(super) fn read_snapshot(
    connection: &Connection,
    session_id: &str,
    config: &crate::configuration::Registry,
) -> Result<Snapshot> {
    let learner = connection.query_row(
        "SELECT id,name,revision,preferences FROM learner WHERE singleton=1",
        [],
        |r| {
            Ok(Learner {
                id: r.get(0)?,
                name: r.get(1)?,
                revision: r.get(2)?,
                preferences: decode(r, 3)?,
            })
        },
    )?;
    config.validate_preferences(&learner.preferences)?;
    let language_profiles = connection
        .prepare("SELECT id,learner_id,language_id FROM language_profiles ORDER BY language_id")?
        .query_map([], |r| {
            Ok(LanguageProfile {
                id: r.get(0)?,
                learner_id: r.get(1)?,
                language_id: r.get(2)?,
            })
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    let personas: Vec<Persona> = connection
        .prepare("SELECT id,learner_id,language_id,revision,details FROM personas ORDER BY rowid")?
        .query_map([], |r| {
            Ok(Persona {
                id: r.get(0)?,
                learner_id: r.get(1)?,
                language_id: r.get(2)?,
                revision: r.get(3)?,
                details: decode(r, 4)?,
            })
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    let contacts = connection
        .prepare("SELECT id,learner_id,persona_id,archived,revision FROM contacts ORDER BY rowid")?
        .query_map([], |r| {
            Ok(Contact {
                id: r.get(0)?,
                learner_id: r.get(1)?,
                persona_id: r.get(2)?,
                archived: r.get(3)?,
                revision: r.get(4)?,
            })
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    let conversations: Vec<Conversation> = connection.prepare("SELECT c.id,c.contact_id,c.language_id,c.title,c.archived,c.revision,c.created_at,MAX(c.last_used,CAST((julianday(c.created_at)-2440587.5)*86400000 AS INTEGER),COALESCE((SELECT CAST((julianday(MAX(m.created_at))-2440587.5)*86400000 AS INTEGER) FROM messages m WHERE m.conversation_id=c.id),0)) AS activity_ms,s.revision,s.settings FROM conversations c JOIN conversation_settings s ON s.conversation_id=c.id ORDER BY activity_ms DESC,c.id DESC")?.query_map([], |r| Ok(Conversation { id:r.get(0)?,contact_id:r.get(1)?,language_id:r.get(2)?,title:r.get(3)?,archived:r.get(4)?,revision:r.get(5)?,created_at:r.get(6)?,last_used:r.get(7)?,settings_revision:r.get(8)?,settings:decode(r,9)? }))?.collect::<rusqlite::Result<Vec<_>>>()?;
    let total: i64 =
        connection.query_row("SELECT count(*) FROM conversations", [], |r| r.get(0))?;
    if total != conversations.len() as i64 {
        return Err(AppError::new(
            ErrorCode::Storage,
            "Conversation settings are missing.",
        ));
    }
    for persona in &personas {
        crate::partners::persona::validate_for_language(
            &persona.details,
            &config.language(&persona.language_id)?,
        )?;
    }
    for conversation in &conversations {
        config.validate_settings(&conversation.language_id, &conversation.settings)?;
    }
    Ok(Snapshot {
        saved_topics: crate::conversations::saved_topics::list(connection)?,
        session_id: session_id.into(),
        revision: connection.query_row(
            "SELECT revision FROM metadata WHERE singleton=1",
            [],
            |r| r.get(0),
        )?,
        learner,
        languages: config.language_projection(),
        language_profiles,
        personas,
        contacts,
        conversations,
    })
}
