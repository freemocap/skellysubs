use super::*;

#[derive(Default)]
pub struct Usage {
    pub attempts: i32,
    pub input: i32,
    pub output: i32,
    pub unknown: i32,
}

impl Runtime {
    /// Count producers, never consumer attempts or observations. Each observation
    /// is cumulative provider information; the latest supplied field wins.
    pub fn usage(
        &self,
        db: &Connection,
        language: Option<&str>,
        persona: Option<&str>,
    ) -> Result<Usage> {
        let rows = db.prepare("SELECT e.conversation_id,w.execution_id,e.id FROM graph_transport_identities w JOIN graph_engines e ON e.id=w.engine_id JOIN conversations c ON c.id=e.conversation_id JOIN contacts r ON r.id=c.contact_id WHERE (?1 IS NULL OR c.language_id=?1) AND (?2 IS NULL OR r.persona_id=?2)")?
            .query_map(params![language,persona],|r|Ok((r.get::<_,String>(0)?,r.get::<_,String>(1)?,r.get::<_,String>(2)?)))?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        let mut total = Usage::default();
        for (_conversation, execution, engine_id) in rows {
            total.attempts += 1;
            let Some(engine) = self
                .engines
                .get(&engine_id)
                .filter(|engine| engine.stamp().engine == engine_id)
            else {
                total.unknown += 1;
                continue;
            };
            let mut reader = graph_store::ReadStore::from_transaction(
                db.unchecked_transaction()?,
                self.partition(&engine_id)?,
            );
            let observations = engine
                .read_execution_evidence(serde_json::from_str(&execution)?, &mut reader)
                .map_err(error)?
                .map_or_else(Vec::new, |e| e.observations);
            let (mut input, mut output) = (None, None);
            for observation in observations {
                if let Some(usage) = observation.usage {
                    if usage.input_tokens.is_some() {
                        input = usage.input_tokens;
                    }
                    if usage.output_tokens.is_some() {
                        output = usage.output_tokens;
                    }
                }
            }
            if input.is_none() || output.is_none() {
                total.unknown += 1;
            }
            let add = |current: i32, count: Option<u64>| -> Result<i32> {
                count
                    .unwrap_or(0)
                    .try_into()
                    .ok()
                    .and_then(|count| current.checked_add(count))
                    .ok_or_else(|| {
                        AppError::new(
                            ErrorCode::Storage,
                            "Graph usage exceeds the report's integer range.",
                        )
                    })
            };
            total.input = add(total.input, input)?;
            total.output = add(total.output, output)?;
        }
        Ok(total)
    }
}
