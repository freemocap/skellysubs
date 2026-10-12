//! Short-lived projections for one locked product read. Reconstruct persisted
//! owners outside the application mutex, then revalidate their exact stamps.
use super::*;
pub struct DetachedViews {
    pub source: crate::ai::inspection::PreparedHistory,
    pub runs: Vec<String>,
}
impl Runtime {
    pub fn detached_views(
        &self,
        db: &Connection,
        conversation: &str,
    ) -> Result<Vec<DetachedViews>> {
        let mut groups = BTreeMap::<String, Vec<String>>::new();
        let mut query=db.prepare("SELECT DISTINCT r.engine_id,r.run_id FROM graph_conversation_runs r JOIN turns t ON t.id=r.turn_id WHERE t.conversation_id=?1 ORDER BY r.engine_id,r.run_id")?;
        for item in query.query_map([conversation], |r| {
            Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?))
        })? {
            let (engine, run) = item?;
            if !self.engines.contains_key(&engine) {
                groups.entry(engine).or_default().push(run);
            }
        }
        groups
            .into_iter()
            .map(|(engine, runs)| {
                Ok(DetachedViews {
                    source: crate::ai::inspection::prepare(db, &engine, &runs[0])?,
                    runs,
                })
            })
            .collect()
    }
    pub fn install_read_views(
        &mut self,
        db: &Connection,
        stamps: &[Stamp],
        views: BTreeMap<String, BTreeMap<String, InspectionSnapshot>>,
    ) -> Result<bool> {
        self.retained_views.clear();
        for expected in stamps {
            use rusqlite::OptionalExtension;
            let actual: Option<String> = db
                .query_row(
                    "SELECT stamp FROM graph_engines WHERE id=?1",
                    [&expected.engine],
                    |r| r.get(0),
                )
                .optional()?;
            if actual
                .map(|s| serde_json::from_str::<Stamp>(&s))
                .transpose()?
                .as_ref()
                != Some(expected)
            {
                return Ok(false);
            }
        }
        self.retained_views = views;
        Ok(true)
    }
    pub fn clear_read_views(&mut self) {
        self.retained_views.clear();
    }
    pub(super) fn retained_view(&self, engine: &str, run: &str) -> Option<InspectionSnapshot> {
        self.retained_views.get(engine)?.get(run).cloned()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        model::{Action, Command},
        storage::store::Store,
    };
    #[test]
    fn detached_product_views_match_retained_history_and_reject_changed_stamps() {
        let dir = tempfile::tempdir().unwrap();
        let mut store = Store::open(&dir.path().join("db")).unwrap();
        store.prepare_chat().unwrap();
        store
            .set_hosted_connection(1, Some("fixture-credential"), "fixture@example.invalid")
            .unwrap();
        let conversation = store.snapshot().unwrap().conversations[0].clone();
        store
            .execute(Command {
                session_id: store.session_id.clone(),
                action_id: uuid::Uuid::new_v4().to_string(),
                action: Action::SendMessage {
                    conversation_id: conversation.id.clone(),
                    text: "Retained source".into(),
                    expected_revision: conversation.revision,
                    input: Default::default(),
                },
            })
            .unwrap();
        // Simulate an older artifact that has no runnable host in this process.
        store.graph_runtime = Runtime::new().unwrap();
        let expected =
            serde_json::to_value(store.conversation_snapshot(&conversation.id, None).unwrap())
                .unwrap();
        let sources = store
            .graph_runtime
            .detached_views(&store.connection, &conversation.id)
            .unwrap();
        assert!(!sources.is_empty());
        let mut stamps = Vec::new();
        let mut views = BTreeMap::new();
        for source in sources {
            let stamp = source.source.source.stamp().clone();
            views.insert(
                stamp.engine.clone(),
                source.source.current_snapshots(&source.runs).unwrap(),
            );
            stamps.push(stamp);
        }
        assert!(
            store
                .graph_runtime
                .install_read_views(&store.connection, &stamps, views.clone())
                .unwrap()
        );
        assert_eq!(
            serde_json::to_value(store.conversation_snapshot(&conversation.id, None).unwrap())
                .unwrap(),
            expected
        );
        store.connection.execute("UPDATE graph_engines SET stamp=json_set(stamp,'$.revision',json_extract(stamp,'$.revision')+1)",[]).unwrap();
        assert!(
            !store
                .graph_runtime
                .install_read_views(&store.connection, &stamps, views)
                .unwrap()
        );
        assert!(store.graph_runtime.retained_views.is_empty());
    }
}
