use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::{
    borrow::Borrow,
    collections::{BTreeMap, BTreeSet},
    ops::Index,
    sync::Arc,
};

/// Stable identities and optional resident payloads. Candidates share loaded
/// records; mutation detaches only its row. Eviction retains identity membership.
/// No transition rules or hidden storage reads are implemented here.
#[derive(Clone, Debug, PartialEq)]
pub(super) struct Records<K: Ord, V> {
    rows: BTreeMap<K, Arc<V>>,
    identities: BTreeSet<K>,
}

impl<K: Ord + Serialize, V: Serialize> Serialize for Records<K, V> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        if self.rows.len() != self.identities.len() {
            return Err(serde::ser::Error::custom(
                "native records require storage reads",
            ));
        }
        self.rows.serialize(serializer)
    }
}

impl<'de, K: Ord + Clone + Deserialize<'de>, V: Deserialize<'de>> Deserialize<'de>
    for Records<K, V>
{
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let rows = BTreeMap::<K, Arc<V>>::deserialize(deserializer)?;
        let identities = rows.keys().cloned().collect();
        Ok(Self { rows, identities })
    }
}

impl<K: Ord, V> Default for Records<K, V> {
    fn default() -> Self {
        Self {
            rows: BTreeMap::new(),
            identities: BTreeSet::new(),
        }
    }
}

impl<K: Ord, V> Records<K, V> {
    pub fn len(&self) -> usize {
        self.identities.len()
    }

    pub fn get<Q: Ord + ?Sized>(&self, key: &Q) -> Option<&V>
    where
        K: Borrow<Q>,
    {
        self.rows.get(key).map(Arc::as_ref)
    }

    pub fn contains_key<Q: Ord + ?Sized>(&self, key: &Q) -> bool
    where
        K: Borrow<Q>,
    {
        self.identities.contains(key)
    }

    pub fn get_shared<Q: Ord + ?Sized>(&self, key: &Q) -> Option<Arc<V>>
    where
        K: Borrow<Q>,
    {
        self.rows.get(key).cloned()
    }

    pub fn insert(&mut self, key: K, value: V)
    where
        K: Clone,
    {
        self.identities.insert(key.clone());
        self.rows.insert(key, Arc::new(value));
    }

    pub fn keys(&self) -> impl DoubleEndedIterator<Item = &K> + ExactSizeIterator {
        self.identities.iter()
    }

    /// Loaded rows only, for native write selection and legacy resident encoding.
    /// Runtime queries enumerate keys and use RecordAccess for payloads.
    pub fn iter(&self) -> impl DoubleEndedIterator<Item = (&K, &V)> + ExactSizeIterator {
        self.rows.iter().map(|(key, value)| (key, value.as_ref()))
    }

    pub fn resident_len(&self) -> usize {
        self.rows.len()
    }

    pub fn evict(&mut self) {
        self.rows.clear();
    }

    pub fn load(&mut self, key: K, row: Arc<V>) {
        assert!(
            self.identities.contains(&key),
            "known native record identity"
        );
        self.rows.entry(key).or_insert(row);
    }
}

impl<K: Ord, V: Clone> Records<K, V> {
    pub fn get_mut<Q: Ord + ?Sized>(&mut self, key: &Q) -> Option<&mut V>
    where
        K: Borrow<Q>,
    {
        self.rows.get_mut(key).map(Arc::make_mut)
    }
}

impl<K, Q, V> Index<&Q> for Records<K, V>
where
    K: Ord + Borrow<Q>,
    Q: Ord + ?Sized,
{
    type Output = V;

    fn index(&self, key: &Q) -> &Self::Output {
        self.get(key).expect("validated native record reference")
    }
}
