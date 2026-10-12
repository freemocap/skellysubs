use crate::model::*;
use crate::storage::store::Store;
use rusqlite::{Connection, params};
fn summary(
    db: &Connection,
    graphs: &crate::conversations::execution::graph_runtime::Runtime,
    id: &str,
    label: &str,
    language: Option<&str>,
    persona: Option<&str>,
) -> Result<UsageSummary> {
    let mut graph_usage = graphs.usage(db, language, persona)?;
    let workspace_usage = crate::ai::workspace_graph::usage(db, language, persona)?;
    graph_usage.attempts += workspace_usage.attempts;
    graph_usage.input += workspace_usage.input;
    graph_usage.output += workspace_usage.output;
    graph_usage.unknown += workspace_usage.unknown;
    let predicate = "(?1 IS NULL OR c.language_id=?1) AND (?2 IS NULL OR r.persona_id=?2)";
    let conversations=db.query_row(&format!("SELECT count(*) FROM conversations c JOIN contacts r ON r.id=c.contact_id WHERE {predicate}"),params![language,persona],|r|r.get(0))?;
    let (learner_messages,persona_messages)=db.query_row(&format!("SELECT COALESCE(SUM(m.role='user'),0),COALESCE(SUM(m.role='assistant'),0) FROM messages m JOIN conversations c ON c.id=m.conversation_id JOIN contacts r ON r.id=c.contact_id WHERE EXISTS(SELECT 1 FROM turn_execution_owners o WHERE o.turn_id=m.turn_id AND o.channel='persona_reply') AND {predicate}"),params![language,persona],|r|Ok((r.get(0)?,r.get(1)?)))?;
    Ok(UsageSummary {
        id: id.into(),
        label: label.into(),
        conversations,
        learner_messages,
        persona_messages,
        attempts: graph_usage.attempts,
        input_tokens: graph_usage.input,
        output_tokens: graph_usage.output,
        unknown_usage: graph_usage.unknown,
    })
}

#[cfg(test)]
mod tests;
impl Store {
    pub fn profile(&self) -> Result<ProfileSnapshot> {
        let snapshot = self.snapshot()?;
        let db = &self.connection;
        let global = summary(
            db,
            &self.graph_runtime,
            "global",
            "All retained activity",
            None,
            None,
        )?;
        let languages = snapshot
            .languages
            .iter()
            .map(|language| {
                summary(
                    db,
                    &self.graph_runtime,
                    &language.id,
                    &language.name,
                    Some(&language.id),
                    None,
                )
            })
            .collect::<Result<Vec<_>>>()?;
        let personas = snapshot
            .personas
            .iter()
            .map(|persona| {
                summary(
                    db,
                    &self.graph_runtime,
                    &persona.id,
                    &persona.details.name,
                    None,
                    Some(&persona.id),
                )
            })
            .collect::<Result<Vec<_>>>()?;
        Ok(ProfileSnapshot {
            revision: snapshot.revision,
            global,
            languages,
            personas,
        })
    }
}
