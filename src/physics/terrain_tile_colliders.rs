use crate::rendering::asset_graph::nodes::adt_node::DoodadReference;
use crate::util::weak_dashmap::WeakKeyDashMapPruneOnInsert;
use rapier3d::prelude::ColliderHandle;

pub struct TerrainTileColliders {
    pub terrain_colliders: Vec<ColliderHandle>,
    /// Doodads that were directly part of the ADT, not those, derived from the WMO. Not the terrain heightmap either.
    pub doodad_colliders: WeakKeyDashMapPruneOnInsert<DoodadReference, ColliderHandle>,
}

impl TerrainTileColliders {
    pub fn new(terrain_colliders: Vec<ColliderHandle>) -> Self {
        Self {
            terrain_colliders,
            doodad_colliders: WeakKeyDashMapPruneOnInsert::new(),
        }
    }
}
