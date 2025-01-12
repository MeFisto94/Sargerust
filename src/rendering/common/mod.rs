/// The game uses far too many coordinate systems, and so we regularly need to transform between them.
/// This module will do so. Note that the convention that we want to use (because it's kind of a middleground), is "blender" (RHS, Z Up, North being +Y)
pub mod coordinate_systems;
/// The objects that are used in the game logic part of the renderer (e.g. MapManager).
/// They represent fully parsed objects, ready to be rendered/transferred into backend specific types.
pub mod highlevel_types;
pub mod mesh_merger;
/// Types that are more specific than the generic render types, but not game logic anymore.
pub mod special_types;
/// basic types (e.g. mesh) to abstract away from both the asset format and the render backend.
pub mod types;
