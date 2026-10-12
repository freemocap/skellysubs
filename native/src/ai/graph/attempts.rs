use super::{AttemptId, AttemptState, ExecutionId, records::Records, state::AttemptRecord};
use serde::{Deserialize, Deserializer, Serialize};
use std::ops::Index;

/// Primary records plus a reconstructible ownership index. Only primary rows
/// are serialized; neither the index nor a caller can redefine row ownership.
#[derive(Clone, Debug, Default, PartialEq, Serialize)]
#[serde(transparent)]
pub(super) struct Attempts {
    rows: Records<AttemptId, AttemptRecord>,
    #[serde(skip)]
    by_run: Records<String, Vec<AttemptId>>,
    #[serde(skip)]
    by_execution: Records<ExecutionId, Vec<AttemptId>>,
}

impl<'de> Deserialize<'de> for Attempts {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let rows = Records::<AttemptId, AttemptRecord>::deserialize(deserializer)?;
        let mut by_run = Records::<String, Vec<AttemptId>>::default();
        let mut by_execution = Records::<ExecutionId, Vec<AttemptId>>::default();
        for (id, record) in rows.iter() {
            if let Some(ids) = by_run.get_mut(&record.run) {
                ids.push(*id);
            } else {
                by_run.insert(record.run.clone(), vec![*id]);
            }
            if let Some(ids) = by_execution.get_mut(&record.attempt.execution) {
                ids.push(*id);
            } else {
                by_execution.insert(record.attempt.execution, vec![*id]);
            }
        }
        Ok(Self {
            rows,
            by_run,
            by_execution,
        })
    }
}

impl Attempts {
    pub fn resident_len(&self) -> usize {
        self.rows.resident_len()
    }
    pub fn evict(&mut self) {
        self.rows.evict();
    }
    pub fn contains_key(&self, id: &AttemptId) -> bool {
        self.rows.contains_key(id)
    }
    pub fn load(&mut self, row: std::sync::Arc<AttemptRecord>) {
        self.rows.load(row.attempt.id, row);
    }
    pub fn execution_ids(&self, execution: ExecutionId) -> impl Iterator<Item = AttemptId> {
        self.by_execution
            .get(&execution)
            .into_iter()
            .flatten()
            .copied()
    }
    pub fn len(&self) -> usize {
        self.rows.len()
    }

    pub fn get(&self, id: &AttemptId) -> Option<&AttemptRecord> {
        self.rows.get(id)
    }

    pub fn get_shared(&self, id: &AttemptId) -> Option<std::sync::Arc<AttemptRecord>> {
        self.rows.get_shared(id)
    }

    pub fn iter(&self) -> impl Iterator<Item = (&AttemptId, &AttemptRecord)> {
        self.rows.iter()
    }

    pub fn insert(&mut self, id: AttemptId, record: AttemptRecord) {
        assert_eq!(id, record.attempt.id, "native attempt identity");
        assert!(
            !self.rows.contains_key(&id),
            "fresh native attempt identity"
        );
        if let Some(ids) = self.by_run.get_mut(&record.run) {
            let position = ids.binary_search(&id).unwrap_err();
            ids.insert(position, id);
        } else {
            self.by_run.insert(record.run.clone(), vec![id]);
        }
        if let Some(ids) = self.by_execution.get_mut(&record.attempt.execution) {
            let position = ids.binary_search(&id).unwrap_err();
            ids.insert(position, id);
        } else {
            self.by_execution.insert(record.attempt.execution, vec![id]);
        }
        self.rows.insert(id, record);
    }

    pub fn set_state(&mut self, id: &AttemptId, state: AttemptState) {
        self.rows
            .get_mut(id)
            .expect("validated native attempt reference")
            .attempt
            .state = state;
    }

    pub fn ids(&self, run: &str) -> &[AttemptId] {
        self.by_run.get(run).map(Vec::as_slice).unwrap_or(&[])
    }

    pub fn for_run(&self, run: &str) -> impl ExactSizeIterator<Item = &AttemptRecord> {
        self.ids(run).iter().map(|id| &self.rows[id])
    }

    #[cfg(test)]
    pub fn for_execution(&self, execution: ExecutionId) -> impl Iterator<Item = &AttemptRecord> {
        self.by_execution
            .get(&execution)
            .into_iter()
            .flatten()
            .map(|id| &self.rows[id])
    }
}

impl Index<&AttemptId> for Attempts {
    type Output = AttemptRecord;

    fn index(&self, id: &AttemptId) -> &Self::Output {
        &self.rows[id]
    }
}
