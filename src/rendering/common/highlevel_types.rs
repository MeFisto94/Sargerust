use crate::rendering::common::types::{Material, MeshWithLod};
use crate::rendering::loader::m2_loader::LoadedM2;
use glam::Affine3A;
use std::sync::Arc;

// TODO: deprecate this file in favor of dedicated graph nodes?

#[derive(Debug, Clone)]
pub struct PlacedDoodad {
    pub transform: Affine3A,

    /// The "loaded" (i.e. parsed and converted into IR) version of the m2 model.
    /// This is an Arc, because models are deduplicated and concurrent loads may have happened
    pub m2: Arc<LoadedM2>,
}

#[derive(Debug, Clone)]
/// A doodad that has been referenced somewhere, but whos M2 is not loaded yet.
pub struct PlaceableDoodad {
    pub transform: Affine3A,
    pub m2_ref: String,
}

#[derive(Clone)]
pub struct PlaceableWMO {
    pub doodads: Vec<PlaceableDoodad>,
    pub loaded_groups: Vec<(MeshWithLod, Vec<Material>)>,
}
