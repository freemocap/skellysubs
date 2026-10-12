//! Bounded encoded display values. Callers own selection and freshness checks.
use std::{collections::VecDeque, sync::Arc};

#[derive(Default)]
pub struct Cache {
    entries: VecDeque<(String, Arc<[u8]>)>,
    bytes: usize,
}
impl Cache {
    const MAX_BYTES: usize = 16 * 1024 * 1024;
    // Include keys and entry/Arc overhead so tiny values cannot grow without bound.
    fn cost(key: &str, value: &[u8]) -> usize {
        key.len()
            + value.len()
            + std::mem::size_of::<(String, Arc<[u8]>)>()
            + 2 * std::mem::size_of::<usize>()
    }
    pub fn get(&mut self, key: &str) -> Option<Arc<[u8]>> {
        let index = self.entries.iter().position(|(k, _)| k == key)?;
        let entry = self.entries.remove(index)?;
        let value = entry.1.clone();
        self.entries.push_back(entry);
        Some(value)
    }
    pub fn insert(&mut self, key: String, value: Vec<u8>) {
        let cost = Self::cost(&key, &value);
        if cost > Self::MAX_BYTES {
            return;
        }
        if let Some(index) = self.entries.iter().position(|(k, _)| k == &key) {
            let (key, value) = self.entries.remove(index).unwrap();
            self.bytes -= Self::cost(&key, &value);
        }
        while self.bytes + cost > Self::MAX_BYTES {
            let (key, value) = self.entries.pop_front().unwrap();
            self.bytes -= Self::cost(&key, &value);
        }
        self.bytes += cost;
        self.entries.push_back((key, value.into()));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn recording_page_remains_warm_on_repeated_sweeps() {
        let mut cache = Cache::default();
        for i in 0..50 {
            cache.insert(format!("receipt-{i}"), vec![0; 1024]);
        }
        for _ in 0..3 {
            for i in 0..50 {
                assert!(cache.get(&format!("receipt-{i}")).is_some());
            }
        }
    }
    #[test]
    fn exact_keys_and_lru_bounds_preserve_recent_reads() {
        let mut cache = Cache::default();
        for i in 0..3 {
            cache.insert(i.to_string(), vec![i as u8; Cache::MAX_BYTES / 4]);
        }
        assert_eq!(cache.get("0").unwrap()[0], 0);
        cache.insert("3".into(), vec![3; Cache::MAX_BYTES / 4]);
        assert!(cache.get("1").is_none());
        assert!(cache.get("0").is_some());
        assert!(cache.get("different-revision").is_none());
        cache.insert(
            "large".into(),
            vec![0; Cache::MAX_BYTES - Cache::cost("large", &[])],
        );
        assert_eq!(cache.entries.len(), 1);
        assert_eq!(cache.bytes, Cache::MAX_BYTES);
    }
}
