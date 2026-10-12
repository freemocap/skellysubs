use super::{ExecutionId, Result, Values, records::Records, state::Execution};
use serde::{Deserialize, Deserializer, Serialize};
use std::{collections::BTreeSet, ops::Index, sync::Arc};

/// Primary producer rows with a derived exact-key index. The index makes no
/// decision about reuse: eligibility and oldest-producer selection belong to
/// the reducer. Identity, work and key cannot be mutated after insertion.
#[derive(Clone, Debug, Default, PartialEq, Serialize)]
#[serde(transparent)]
pub(super) struct Executions {
    rows: Records<ExecutionId, Execution>,
    #[serde(skip)]
    by_key: Records<String, Vec<ExecutionId>>,
    #[serde(skip)]
    unresolved: Arc<BTreeSet<ExecutionId>>,
}

impl<'de> Deserialize<'de> for Executions {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> std::result::Result<Self, D::Error> {
        let rows = Records::<ExecutionId, Execution>::deserialize(deserializer)?;
        let mut by_key = Records::<String, Vec<ExecutionId>>::default();
        let mut unresolved = BTreeSet::new();
        for (id, record) in rows.iter() {
            if record.outcome.is_none() && !record.unknown {
                unresolved.insert(*id);
            }
            if let Some(ids) = by_key.get_mut(&record.key) {
                ids.push(*id);
            } else {
                by_key.insert(record.key.clone(), vec![*id]);
            }
        }
        Ok(Self {
            rows,
            by_key,
            unresolved: Arc::new(unresolved),
        })
    }
}

impl Executions {
    pub fn resident_len(&self) -> usize {
        self.rows.resident_len()
    }
    pub fn evict(&mut self) {
        self.rows.evict();
    }
    pub fn contains_key(&self, id: &ExecutionId) -> bool {
        self.rows.contains_key(id)
    }
    pub fn load(&mut self, row: Arc<Execution>) {
        self.rows.load(row.work.execution, row);
    }
    pub fn unresolved_ids(
        &self,
    ) -> impl DoubleEndedIterator<Item = ExecutionId> + ExactSizeIterator {
        self.unresolved.iter().copied()
    }
    pub fn key_ids(&self, key: &str) -> impl Iterator<Item = ExecutionId> {
        self.by_key.get(key).into_iter().flatten().copied()
    }
    pub fn len(&self) -> usize {
        self.rows.len()
    }

    pub fn get(&self, id: &ExecutionId) -> Option<&Execution> {
        self.rows.get(id)
    }

    pub fn get_shared(&self, id: &ExecutionId) -> Option<Arc<Execution>> {
        self.rows.get_shared(id)
    }

    pub fn iter(
        &self,
    ) -> impl DoubleEndedIterator<Item = (&ExecutionId, &Execution)> + ExactSizeIterator {
        self.rows.iter()
    }

    /// Exact projection of outcome=None and unknown=false, in native ID order.
    /// It does not encode resource admission or consumer eligibility.
    #[cfg(test)]
    pub fn unresolved(
        &self,
    ) -> impl DoubleEndedIterator<Item = (ExecutionId, &Execution)> + ExactSizeIterator {
        self.unresolved.iter().map(|id| (*id, &self.rows[id]))
    }

    #[cfg(test)]
    pub fn with_key(&self, key: &str) -> impl Iterator<Item = (ExecutionId, &Execution)> {
        self.by_key
            .get(key)
            .into_iter()
            .flatten()
            .map(|id| (*id, &self.rows[id]))
    }

    pub fn insert(&mut self, id: ExecutionId, record: Execution) {
        assert_eq!(id, record.work.execution, "native execution identity");
        assert!(
            !self.rows.contains_key(&id),
            "fresh native execution identity"
        );
        if let Some(ids) = self.by_key.get_mut(&record.key) {
            let position = ids.binary_search(&id).unwrap_err();
            ids.insert(position, id);
        } else {
            self.by_key.insert(record.key.clone(), vec![id]);
        }
        if record.outcome.is_none() && !record.unknown {
            Arc::make_mut(&mut self.unresolved).insert(id);
        }
        self.rows.insert(id, record);
    }

    pub fn set_dispatched(&mut self, id: &ExecutionId) {
        self.rows
            .get_mut(id)
            .expect("validated native execution reference")
            .dispatched = true;
    }

    pub fn set_outcome(&mut self, id: &ExecutionId, outcome: Result<Values>) {
        self.rows
            .get_mut(id)
            .expect("validated native execution reference")
            .outcome = Some(outcome);
        Arc::make_mut(&mut self.unresolved).remove(id);
    }

    pub fn set_unknown(&mut self, id: &ExecutionId) {
        self.rows
            .get_mut(id)
            .expect("validated native execution reference")
            .unknown = true;
        Arc::make_mut(&mut self.unresolved).remove(id);
    }

    pub fn set_evidence(&mut self, id: &ExecutionId, evidence: super::ExecutionEvidence) {
        self.rows
            .get_mut(id)
            .expect("validated native execution reference")
            .evidence = Some(evidence);
    }
}

impl Index<&ExecutionId> for Executions {
    type Output = Execution;

    fn index(&self, id: &ExecutionId) -> &Self::Output {
        &self.rows[id]
    }
}
