use std::collections::HashMap;
use std::ops::DerefMut;
use std::sync::{Arc, RwLock, RwLockWriteGuard, Weak};

pub struct Resolver<G: GraphNodeGenerator<T>, T> {
    ref_cache: RwLock<HashMap<String, RwLock<Weak<T>>>>,
    generator: G,
}

pub trait GraphNodeGenerator<T> {
    fn generate(&self, name: &str) -> Arc<T>;
}

impl<G: GraphNodeGenerator<T>, T> Resolver<G, T> {
    pub fn new(generator: G) -> Self {
        Self {
            ref_cache: RwLock::new(HashMap::new()),
            generator,
        }
    }

    // TODO: maybe take name by reference and only own it when inserting
    pub fn resolve(&self, name: String) -> Arc<T> {
        // Easy path: The cache contains a weak reference
        if let Some(weak_lock) = self
            .ref_cache
            .read()
            .expect("Get the read lock on the cache")
            .get(&name)
        {
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
        {
            let mut wlock = self
                .ref_cache
                .write()
                .expect("Get the write lock on the cache");
            wlock.insert(name.clone(), RwLock::new(Weak::new()));
        }

        {
            let rlock = self
                .ref_cache
                .read()
                .expect("Get the read lock on the cache");
            let mut weak = rlock
                .get(&name)
                .unwrap()
                .write()
                .expect("Get the write lock on the entry");
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
