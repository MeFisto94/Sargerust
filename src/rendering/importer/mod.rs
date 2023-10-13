pub mod adt_importer;
/// This module will handle converting the types from sargerust-files into a intermediate representation,
/// that can then be used to build actual Meshes, Materials and Objects.
/// This abstraction is made to be a) render backend crate agnostic and b) allow for caching of the
/// intermediate representation (as opposed to the parsed assets), so the meshes can be derived
/// multiple times (especially also for instancing), without directly working in the complexity
/// that is the asset files themselves.
pub mod m2_importer;
pub mod wmo_importer;
