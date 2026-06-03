use std::hash::Hash;

use rustc_hash::FxHashSet;

pub struct UniqVec<T, K> {
    items: Vec<T>,
    seen: FxHashSet<K>,
}

impl<T, K: Eq + Hash> Default for UniqVec<T, K> {
    fn default() -> Self {
        Self { items: Vec::new(), seen: FxHashSet::default() }
    }
}

impl<T, K: Eq + Hash + Clone> UniqVec<T, K> {
    pub fn push(&mut self, keys: impl IntoIterator<Item = K>, value: T) -> bool {
        let keys = keys.into_iter().collect::<Vec<_>>();
        if keys.iter().any(|key| self.seen.contains(key)) {
            return false;
        }

        self.seen.extend(keys);
        self.items.push(value);
        true
    }

    pub fn contains(&self, key: &K) -> bool {
        self.seen.contains(key)
    }

    pub fn get(&self, idx: usize) -> &T {
        &self.items[idx]
    }

    pub fn len(&self) -> usize {
        self.items.len()
    }

    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    pub fn into_vec(self) -> Vec<T> {
        self.items
    }
}
