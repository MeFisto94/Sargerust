use dashmap::DashMap;
use itertools::Itertools;
use std::sync::Weak;

struct MapValue<K, V> {
    weak: Weak<K>,
    value: V,
}

/// A special form of a concurrent hash map that is optimized to store values that are derived from a Weak<T>.
/// They thus allow freeing of the Arc<T> and are used for derivative data that tracks T, but also doesn't need to exist
/// anymore when T is gone. Internally it uses the actual Weak pointers' address as the hash key and some logic to
/// automatically prune expired weaks. Compare it to https://docs.rs/weak-table/latest/weak_table/, but it additionally
/// is based on DashMap to allow for interior mutability.
pub struct WeakKeyDashMapPruneOnInsert<K, V> {
    inner: DashMap<*const K, MapValue<K, V>>,
}

unsafe impl<K, V> Send for WeakKeyDashMapPruneOnInsert<K, V> {}

impl<K, V> WeakKeyDashMapPruneOnInsert<K, V> {
    pub fn new() -> Self {
        Self {
            inner: DashMap::new(),
        }
    }

    pub fn with_capacity(capacity: usize) -> Self {
        Self {
            inner: DashMap::with_capacity(capacity),
        }
    }

    #[inline]
    pub fn insert(&self, key: Weak<K>, value: V) {
        self.try_prune();

        let ptr = key.as_ptr();
        let value = MapValue { weak: key, value };

        self.inner.insert(ptr, value);
    }

    #[inline]
    pub fn contains_key(&self, key: Weak<K>) -> bool {
        self.inner.contains_key(&key.as_ptr())
    }

    #[inline]
    pub fn compute_if_absent<F>(&self, key: Weak<K>, compute: F)
    where
        F: FnOnce(&Weak<K>) -> V,
    {
        let ptr = key.as_ptr();
        if self.inner.contains_key(&ptr) {
            return;
        }

        let value = compute(&key);
        self.insert(key, value);
    }

    #[inline]
    fn try_prune(&self) {
        // Would have to allocate after the next insert.
        if self.inner.capacity() == self.inner.len() {
            self.inner.retain(|_, v| v.weak.strong_count() > 0);
        }
    }

    #[inline]
    pub fn shrink_to_fit(&self) {
        self.inner.shrink_to_fit()
    }
}

impl<K, V> Default for WeakKeyDashMapPruneOnInsert<K, V> {
    fn default() -> Self {
        Self::new()
    }
}

impl<K, V: Clone> WeakKeyDashMapPruneOnInsert<K, V> {
    // TODO: This could be implemented with IntoIter etc but I am failing the generics atm.
    pub(crate) fn values(&self) -> Vec<V> {
        self.inner.iter().map(|v| v.value.clone()).collect_vec()
    }
}
