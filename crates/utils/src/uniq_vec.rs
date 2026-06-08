use std::hash::Hash;

use rustc_hash::FxHashSet;

#[derive(Debug, Clone)]
pub struct UniqVec<T, K> {
    items: Vec<T>,
    seen: FxHashSet<K>,
}

impl<T: PartialEq, K: Eq + Hash> PartialEq for UniqVec<T, K> {
    fn eq(&self, other: &Self) -> bool {
        self.items == other.items && self.seen == other.seen
    }
}

impl<T: Eq, K: Eq + Hash> Eq for UniqVec<T, K> {}

impl<T, K: Eq + Hash> Default for UniqVec<T, K> {
    fn default() -> Self {
        Self { items: Vec::new(), seen: FxHashSet::default() }
    }
}

impl<T, K: Eq + Hash> UniqVec<T, K> {
    pub fn push(&mut self, keys: impl IntoIterator<Item = K>, value: T) -> bool {
        let keys = keys.into_iter().collect::<Vec<_>>();
        if keys.iter().any(|key| self.seen.contains(key)) {
            return false;
        }

        self.seen.extend(keys);
        self.items.push(value);
        true
    }

    pub fn push_keyed<F>(&mut self, value: T, key: F) -> bool
    where
        F: FnOnce(&T) -> K,
    {
        let key = key(&value);
        self.push([key], value)
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

    pub fn as_slice(&self) -> &[T] {
        &self.items
    }

    pub fn into_vec(self) -> Vec<T> {
        self.items
    }
}

impl<T: Clone + Eq + Hash> UniqVec<T, T> {
    pub fn push_unique(&mut self, value: T) -> bool {
        self.push([value.clone()], value)
    }
}

impl<T: PartialEq> UniqVec<T, ()> {
    pub fn push_unique_by<F>(&mut self, value: T, same: F) -> bool
    where
        F: Fn(&T, &T) -> bool,
    {
        if self.items.iter().any(|existing| same(existing, &value)) {
            return false;
        }
        self.items.push(value);
        true
    }

    pub fn push_unique_eq(&mut self, value: T) -> bool {
        self.push_unique_by(value, |existing, value| existing == value)
    }
}
