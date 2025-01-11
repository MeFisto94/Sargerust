use crate::rendering::asset_graph::nodes::adt_node::DoodadReference;
use rapier3d::prelude::ColliderHandle;
use std::sync::{Arc, RwLock, Weak};

// TODO: This technically belongs somewhere else, as it's not terrain tile specific.
pub struct DoodadColliderEntry {
    pub reference: Weak<DoodadReference>,
    pub collider_handle: ColliderHandle,
}

pub struct TerrainTileColliders {
    pub terrain_colliders: Vec<ColliderHandle>,
    /// Doodads that were directly part of the ADT, not those, derived from the WMO. Not the terrain heightmap either.
    // sadly, Weaks can't be used as Hash/DashMap keys, otherwise we would've used it.
    pub doodad_colliders: Arc<RwLock<Vec<DoodadColliderEntry>>>,
}

impl TerrainTileColliders {
    pub fn new(terrain_colliders: Vec<ColliderHandle>) -> Self {
        Self {
            terrain_colliders,
            doodad_colliders: Arc::new(RwLock::new(vec![])),
        }
    }
}
