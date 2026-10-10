use std::collections::{HashMap, VecDeque};

/// Entries keyed by id, capped at `cap`. Inserting an id makes it the newest
/// entry, and the oldest entries are evicted past the cap. The eviction order
/// is private, so the map and its order cannot drift apart.
pub(in crate::app) struct BoundedCache<V> {
    cap: usize,
    entries: HashMap<String, V>,
    /// Keys oldest first. Holds exactly the keys of `entries`, once each.
    order: VecDeque<String>,
}

impl<V> BoundedCache<V> {
    pub(in crate::app) fn new(cap: usize) -> Self {
        Self {
            cap,
            entries: HashMap::new(),
            order: VecDeque::new(),
        }
    }

    pub(in crate::app) fn insert(&mut self, key: String, value: V) {
        self.order.retain(|known| *known != key);
        self.order.push_back(key.clone());
        self.entries.insert(key, value);
        let excess = self.order.len().saturating_sub(self.cap);
        for oldest in self.order.drain(..excess) {
            self.entries.remove(&oldest);
        }
    }

    pub(in crate::app) fn get(&self, key: &str) -> Option<&V> {
        self.entries.get(key)
    }

    pub(in crate::app) fn get_mut(&mut self, key: &str) -> Option<&mut V> {
        self.entries.get_mut(key)
    }

    pub(in crate::app) fn contains_key(&self, key: &str) -> bool {
        self.entries.contains_key(key)
    }

    pub(in crate::app) fn values(&self) -> impl Iterator<Item = &V> {
        self.entries.values()
    }

    #[cfg(test)]
    pub(in crate::app) fn len(&self) -> usize {
        self.entries.len()
    }

    #[cfg(test)]
    pub(in crate::app) fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub(in crate::app) fn clear(&mut self) {
        self.entries.clear();
        self.order.clear();
    }
}

impl<V> std::ops::Index<&str> for BoundedCache<V> {
    type Output = V;

    fn index(&self, index: &str) -> &V {
        &self.entries[index]
    }
}
