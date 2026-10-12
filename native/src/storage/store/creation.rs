use super::*;

/// The one place a persona and its contact are created, from the starter or from
/// a generated result. The language is validated here.
pub(super) fn create_persona(
    db: &Connection,
    registry: &crate::configuration::Registry,
    learner: &str,
    language_id: &str,
    details: PersonaDetails,
) -> Result<(String, String)> {
    crate::partners::persona::validate_for_language(&details, &registry.language(language_id)?)?;
    let persona_id = id();
    let contact_id = id();
    db.execute("INSERT INTO language_profiles SELECT ?1,?2,?3 WHERE NOT EXISTS(SELECT 1 FROM language_profiles WHERE learner_id=?2 AND language_id=?3)", params![id(), learner, language_id])?;
    db.execute(
        "INSERT INTO personas VALUES(?1,?2,?3,1,?4)",
        params![
            persona_id,
            learner,
            language_id,
            serde_json::to_string(&details)?
        ],
    )?;
    db.execute(
        "INSERT INTO contacts VALUES(?1,?2,?3,0,1)",
        params![contact_id, learner, persona_id],
    )?;
    Ok((persona_id, contact_id))
}

/// The one place a conversation is created for a contact.
pub(super) fn create_conversation(
    db: &Connection,
    contact_id: &str,
    language_id: &str,
    title: &str,
    settings: &PracticeSettings,
) -> Result<String> {
    #[cfg(feature = "workflow-capture")]
    let conversation_id = crate::ai::transport::workflow::conversation_identity()?;
    #[cfg(not(feature = "workflow-capture"))]
    let conversation_id = id();
    db.execute("INSERT INTO conversations(id,contact_id,language_id,title,archived,revision,last_used) VALUES(?1,?2,?3,?4,0,1,MAX(CAST((julianday('now')-2440587.5)*86400000 AS INTEGER),COALESCE((SELECT MAX(last_used) FROM conversations),0)+1))", params![conversation_id, contact_id, language_id, title])?;
    db.execute(
        "INSERT INTO conversation_settings VALUES(?1,1,?2)",
        params![conversation_id, serde_json::to_string(settings)?],
    )?;
    Ok(conversation_id)
}
