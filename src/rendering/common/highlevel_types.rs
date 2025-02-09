use glam::Affine3A;

// TODO: deprecate this file in favor of dedicated graph nodes?

#[derive(Debug, Clone)]
/// A doodad that has been referenced somewhere, but whos M2 is not loaded yet.
pub struct PlaceableDoodad {
    pub transform: Affine3A,
    pub m2_ref: String,
    pub set_name: String,
}
