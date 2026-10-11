//! Private derived inspection responses. Exact committed stamp and selection are
//! the key; callers must revalidate ownership before every lookup.
use std::{collections::VecDeque, sync::Arc};

#[derive(Default)]
pub struct Cache {
    entries: VecDeque<(String, Arc<[u8]>)>,
    bytes: usize,
}
impl Cache {
    const MAX_BYTES: usize = 16 * 1024 * 1024;
    pub fn get(&mut self, key: &str) -> Option<Arc<[u8]>> {
        let index = self.entries.iter().position(|(k, _)| k == key)?;
        let entry = self.entries.remove(index)?;
        let value = entry.1.clone();
        self.entries.push_back(entry);
        Some(value)
    }
    pub fn insert(&mut self, key: String, value: Vec<u8>) {
        if value.len() > Self::MAX_BYTES {
            return;
        }
        if let Some(index) = self.entries.iter().position(|(k, _)| k == &key) {
            self.bytes -= self.entries.remove(index).unwrap().1.len();
        }
        while self.entries.len() >= 32 || self.bytes + value.len() > Self::MAX_BYTES {
            self.bytes -= self.entries.pop_front().unwrap().1.len();
        }
        self.bytes += value.len();
        self.entries.push_back((key, value.into()));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn exact_keys_and_lru_bounds_preserve_recent_reads() {
        let mut cache = Cache::default();
        for i in 0..32 {
            cache.insert(i.to_string(), vec![i as u8]);
        }
        assert_eq!(&*cache.get("0").unwrap(), &[0]);
        cache.insert("32".into(), vec![32]);
        assert!(cache.get("1").is_none());
        assert!(cache.get("0").is_some());
        assert!(cache.get("different-revision").is_none());
        cache.insert("large".into(), vec![0; Cache::MAX_BYTES]);
        assert_eq!(cache.entries.len(), 1);
        assert_eq!(cache.bytes, Cache::MAX_BYTES);
    }
}
