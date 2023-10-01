/// The game uses far too many coordinate systems, and so we regularly need to transform between them.
/// This module will do so. Note that the convention that we want to use (because it's kind of a middleground), is "blender" (RHS, Z Up, North being +Y)
pub mod coordinate_systems;