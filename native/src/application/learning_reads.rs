//! Workspace-lifetime reuse of read-only learner display projections.
use super::{workspace_version::Version, *};
use crate::learning::{effort, learner::progression};
use serde::{Serialize, de::DeserializeOwned};

#[derive(Default)]
pub(super) struct Cache {
    version: Option<Version>,
    values: super::response_cache::Cache,
}
impl Cache {
    fn observe(&mut self, version: &Version) {
        if self.version.as_ref() != Some(version) {
            self.values = Default::default();
            self.version = Some(version.clone());
        }
    }

    pub(super) fn navigation_committed(&mut self, before: &Version, after: &Version) {
        if self.version.as_ref() == Some(before) {
            self.version = Some(after.clone());
        }
    }
}

#[derive(Serialize)]
enum Selection<'a> {
    Evidence(&'a str),
    Totals,
    Effort(&'a str, Option<&'a str>),
}

impl Application {
    // Selection and result type are paired only by the three entry points below.
    // Execution and credit publication continue to use authoritative store reads.
    fn learning_read<T: Serialize + DeserializeOwned>(
        &self,
        selection: Selection<'_>,
        read: impl FnOnce(&Store) -> Result<T>,
    ) -> Result<T> {
        let key = serde_json::to_string(&selection)?;
        let (version, value) = {
            let store = self.lock()?;
            let version = Version::of(&store);
            let cached = {
                let mut cache = self.learning_reads.lock().map_err(|_| internal())?;
                cache.observe(&version);
                cache.values.get(&key)
            };
            if let Some(bytes) = cached {
                drop(store);
                return Ok(serde_json::from_slice(&bytes)?);
            }
            let value = read(&store)?;
            (version, value)
        };
        // Encoding and warm decoding do not hold the workspace lock. Never
        // install a projection if a write occurred while encoding it.
        let bytes = serde_json::to_vec(&value)?;
        let store = self.lock()?;
        if Version::of(&store) == version {
            let mut cache = self.learning_reads.lock().map_err(|_| internal())?;
            cache.observe(&version);
            cache.values.insert(key, bytes);
        }
        Ok(value)
    }

    pub(crate) fn read_skill_evidence(&self, target: &str) -> Result<serde_json::Value> {
        self.learning_read(Selection::Evidence(target), |store| {
            progression::snapshot(store, target)
        })
    }

    pub(crate) fn read_language_totals(&self) -> Result<Vec<progression::LanguageTotals>> {
        self.learning_read(Selection::Totals, progression::language_totals)
    }

    pub(crate) fn read_effort_progress(
        &self,
        target: &str,
        conversation: Option<&str>,
    ) -> Result<effort::EffortProgress> {
        self.learning_read(Selection::Effort(target, conversation), |store| {
            store.config.language(target)?;
            effort::read_scoped(&store.connection, target, conversation)
        })
    }
}

#[cfg(test)]
#[path = "tests/learning_reads.rs"]
mod tests;
