use std::sync::{Arc, Weak};

use dashmap::DashMap;
use dashmap::mapref::entry::Entry;

pub struct Resolver<G: GraphNodeGenerator<T>, T> {
    ref_cache: DashMap<String, Weak<T>>,
    generator: G,
}

pub trait GraphNodeGenerator<T> {
    fn generate(&self, name: &str) -> Arc<T>;
}

impl<G: GraphNodeGenerator<T>, T> Resolver<G, T> {
    pub fn new(generator: G) -> Self {
        Self {
            // One could also increase the shard amount, but nbThreads * 4 is the default, so ~32 on
            // a Quad Core. That pairs well with probably < 100 entries
            ref_cache: DashMap::with_capacity(100),
            generator,
        }
    }

    // TODO: maybe take name by reference and only own it when inserting.
    //  also canonicalize paths: uppercase and forward slashes as in MPQ?
    //  -> Those two requirements do conflict, though.
    pub fn resolve(&self, name: String) -> Arc<T> {
        // optimistic path
        // can be removed without impacting correctness
        if let Some(existing) = self.ref_cache.get(&name).and_then(|x| x.upgrade()) {
            return existing;
        }

        // TODO: this may or may not be a performance culprit in the future. Move generate in or out
        //  and fine tune the shard size.
        // assume that nobody else is trying to initialize this entry
        // benefit: we can call `generate` outside of the critical section
        // drawback: if we're wrong, `generate` gets called more than once
        // let new = self.generator.generate(&name);

        // clone can be removed, when generating outside the critical section
        match self.ref_cache.entry(name.clone()) {
            Entry::Occupied(mut o) => {
                if let Some(existing) = o.get().upgrade() {
                    // the optimistic path failed earlier,
                    // but someone slipped an entry in since then
                    existing
                } else {
                    // there was already an entry, but it died
                    let new = self.generator.generate(&name);
                    o.insert(Arc::downgrade(&new));
                    new
                }
            }
            Entry::Vacant(v) => {
                // there was no entry
                let new = self.generator.generate(&name);
                v.insert(Arc::downgrade(&new));
                new
            }
        }
    }
}
