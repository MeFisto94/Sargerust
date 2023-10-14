use dashmap::DashMap;
use std::ops::DerefMut;
use std::sync::{Arc, RwLock, RwLockWriteGuard, Weak};

pub struct Resolver<G: GraphNodeGenerator<T>, T> {
    ref_cache: DashMap<String, RwLock<Weak<T>>>,
    generator: G,
}

pub trait GraphNodeGenerator<T> {
    fn generate(&self, name: &str) -> Arc<T>;
}

impl<G: GraphNodeGenerator<T>, T> Resolver<G, T> {
    pub fn new(generator: G) -> Self {
        Self {
            ref_cache: DashMap::with_capacity(100),
            generator,
        }
    }

    // TODO: maybe take name by reference and only own it when inserting.
    //  also canonicalize paths: uppercase and forward slashes as in MPQ?
    //  -> Those two requirements do conflict, though.
    pub fn resolve(&self, name: String) -> Arc<T> {
        // TODO: This is one of the hottest paths when loading assets, due to the locking on the hashmap
        //  consider experimenting with alternatives (chashmap, dashmap, leapfrog). However,
        //  according to the benchmark from the dashmap author (https://github.com/xacrimon/conc-map-bench)
        //  dashmap should be very competetive and outperform chashmap.
        // Easy path: The cache contains a weak reference
        if let Some(weak_lock) = self.ref_cache.get(&name) {
            // TODO: with dashmap, could we also just get_mut instead of having RwLocks inside the entries?
            {
                let weak = weak_lock.read().expect("Get the read lock on the entry");
                if let Some(arc) = weak.upgrade() {
                    return arc;
                }
            }
            {
                let mut weak = weak_lock.write().expect("Get the write lock on the entry");
                return Self::generate(self, &name, &mut weak);
            }
        }

        // Heavier path: We need to lock the entire cache to insert a new weak reference, however
        // we try to do this as short as possible by _not_ waiting for the generator.
        // Note: the above comment is kind-of outdated with DashMap but was true for RwLock<HashMap<_>>
        // the reasoning still remains, though, as we just lock on smaller buckets but still do lock.
        {
            self.ref_cache
                .insert(name.clone(), RwLock::new(Weak::new()));
        }

        {
            let entry = self.ref_cache.get(&name).unwrap();
            let mut weak = entry.write().expect("Get the write lock on the entry");
            Self::generate(self, &name, &mut weak)
        }
    }

    fn generate(&self, name: &str, weak: &mut RwLockWriteGuard<Weak<T>>) -> Arc<T> {
        match weak.upgrade() {
            Some(arc) => arc, // maybe we have been raced
            None => {
                let arc = self.generator.generate(name);
                *weak.deref_mut() = Arc::downgrade(&arc.clone());
                arc
            }
        }
    }
}
