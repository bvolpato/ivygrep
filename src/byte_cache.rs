use std::hash::Hash;
use std::num::NonZeroUsize;

use lru::LruCache;

/// Bound retained payloads by bytes and entries. Values carry their own cost.
pub(crate) struct ByteCache<K, V> {
    entries: LruCache<K, (V, usize)>,
    bytes: usize,
    max_bytes: usize,
}

impl<K: Hash + Eq, V> ByteCache<K, V> {
    pub(crate) fn new(max_entries: usize, max_bytes: usize) -> Self {
        Self {
            entries: LruCache::new(NonZeroUsize::new(max_entries).unwrap()),
            bytes: 0,
            max_bytes,
        }
    }

    pub(crate) fn get<Q: Hash + Eq + ?Sized>(&mut self, key: &Q) -> Option<&V>
    where
        K: std::borrow::Borrow<Q>,
    {
        self.entries.get(key).map(|(value, _)| value)
    }

    pub(crate) fn fits(&self, bytes: usize) -> bool {
        bytes <= self.max_bytes
    }

    pub(crate) fn insert(&mut self, key: K, value: V, bytes: usize) {
        self.remove(&key);
        if !self.fits(bytes) {
            return;
        }
        // Evict before adding, so the accounting cannot overflow.
        while self.bytes > self.max_bytes - bytes {
            if let Some((_, (_, removed))) = self.entries.pop_lru() {
                self.bytes -= removed;
            }
        }
        if let Some((_, (_, removed))) = self.entries.push(key, (value, bytes)) {
            self.bytes -= removed;
        }
        self.bytes += bytes;
    }

    pub(crate) fn remove(&mut self, key: &K) {
        if let Some((_, bytes)) = self.entries.pop(key) {
            self.bytes -= bytes;
        }
    }

    pub(crate) fn keys(&self) -> impl Iterator<Item = &K> {
        self.entries.iter().map(|(key, _)| key)
    }

    #[cfg(test)]
    pub(crate) fn contains(&self, key: &K) -> bool {
        self.entries.contains(key)
    }

    #[cfg(test)]
    pub(crate) fn len(&self) -> usize {
        self.entries.len()
    }

    #[cfg(test)]
    pub(crate) fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn byte_pressure_evicts_cold_entries_and_replacement_releases_bytes() {
        let mut cache = ByteCache::new(8, 10);
        cache.insert(1, "first", 4);
        cache.insert(2, "second", 4);
        assert_eq!(cache.get(&1), Some(&"first"));
        cache.insert(3, "third", 4);
        assert!(!cache.contains(&2));
        assert_eq!(cache.bytes, 8);
        cache.insert(1, "replacement", 2);
        assert_eq!(cache.bytes, 6);
        cache.remove(&3);
        assert_eq!(cache.bytes, 2);
        cache.insert(1, "oversized", 11);
        assert_eq!(cache.bytes, 0);
        assert_eq!(cache.len(), 0);
    }

    #[test]
    fn entry_eviction_and_zero_budget_keep_accounting_exact() {
        let mut cache = ByteCache::new(1, 10);
        cache.insert(1, (), 2);
        cache.insert(2, (), 3);
        assert_eq!(cache.bytes, 3);
        assert!(!cache.contains(&1));
        let mut disabled = ByteCache::new(1, 0);
        disabled.insert(1, (), 1);
        assert_eq!(disabled.len(), 0);
    }
}
