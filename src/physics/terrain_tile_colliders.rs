use crate::rendering::asset_graph::nodes::adt_node::{M2Node, WMONode};
use rapier3d::prelude::ColliderHandle;
use std::sync::{Arc, Mutex, Weak};

pub struct TerrainTileColliders {
    pub terrain_colliders: Vec<ColliderHandle>,
    /// directly added doodads. Not those, derived from the WMO.
    pub doodad_colliders: Arc<Mutex<Vec<(Weak<M2Node>, ColliderHandle)>>>,
}

impl TerrainTileColliders {
    pub fn new(terrain_colliders: Vec<ColliderHandle>) -> Self {
        Self {
            terrain_colliders,
            doodad_colliders: Arc::new(Mutex::new(vec![])),
        }
    }
}
