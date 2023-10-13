//! This module contains the directional acyclic graph that is built to keep track of dependencies
//! and to provide asset deduplication, streaming ("async loading") and GPU memory management.
//!
//! To achieve the first goal of deduplication, the graph will mostly consist of [`std::sync::Arc`]s.
//! Whoever has the first reference to any specific asset takes care of parsing and importing
//! (i.e. [`rendering::loader`]) it and storing it behind an [`std::sync::Arc`]. There may be some
//! locking process, because in the end, the imported asset needs to be in some sort of lookup
//! storage, where future reference resolutions will [`std::sync::Arc::clone()`] from.
//!
//! The goal of streaming comes in naturally, provided the graph's reference resolution is thread-safe.
//! All that has to happen is enqueuing a loader once per encountered reference, while either
//! preventing further loader invocations _or_ ensuring that the raced computation result is dropped
//! and instead the Arc of the faster loader is used, essentially discarding the result, otherwise
//! deduplication will NOT function properly.
//!
//! Hint: Considering how typical assets use the same doodad and textures in sequence, the first
//! approach is a lot more reasonable.
//!
//! Implementing GPU Memory Management is also an implicit benefit from this graph (and streaming),
//! at least when using the rend3 backend. There, we have [`rend3::types::ResourceHandle`]s, that
//! kind of act like a refcounted handle into GPU Memory. Whenever the relevant handle is [`Drop`]ped,
//! the GPU Memory will be freed.
//!
//! Note: Another technique, that is not implemented yet, would be "node hollowing": As soon as any
//! given node has a [`rend3::types::ResourceHandle`], it's IR could be freed, because relevant
//! drawing information is stored on the GPU. Doing so will reduce RAM Usage (technically the "whole"
//! VRAM (without costly framebuffers, though) will be mirrored in your RAM), but it comes at the
//! expense of slower re-loading, whenever the handle had been dropped and has to be restored.
//! This is especially the case with meshes/index buffers, as happens when the LoD level changes.
//! In most other cases, the handle is only dropped when the node itself has been dropped anyway.
//!
//! Note: Another technique, that is not implemented yet, is "tree pruning": Technically, the game
//! only needs to know which IR/Handles belong to which terrain tile, so they can be [`Drop`]ped
//! whenever the tile is unloaded, as that will get rid of the complete chain down to the handles
//! that are not referenced by any other tile anyway, and so intermediary nodes could be pruned
//! after all references are resolved, moving the [`std::sync::Arc`]s into the Terrain Tile Node.
//! This also reduces the memory usage, because we can get rid of currently inactive variants, but
//! this likewise comes at the expense of slower re-loading, too.
//! When things happen like the selected WMO Groups change (e.g. due to bounding box culling), the
//! game assets need to be loaded and parsed again (the full tree, with the exception of textures)
//! and the [`rend3::types::ResourceHandle`]s need to be reconstructed carefully.
//!
//! Note: Not all references need to be fully loaded, there are cases like WMO Groups and LOD Levels
//! where it's expected to load things lazy (however [`rendering::common::types::MeshWithLod`] could
//! be used to keep all LoD Levels in one mesh), but the renderer also has some freedoms, e.g. using
//! frustum culling and in general only loading (and potentially dropping) some resources based on
//! distance (don't load far away WMO Groups or M2s).
//!
//! If you want further understand the graph, have a look at `docs/AssetsDAG.png` that visualizes
//! the graph that is built. Have a look at the legend to the left, explaining the different node
//! types, where IR is typically backed by a resource handle.
//!
//! The thin arrows represent references, which, when resolved, are [`std::sync::Arc`]s to the
//! following nodes. WMO references are a speciality (dashed line), because they may be referenced
//! from multiple ADT files, but still must only exist once. There's a specific uniqueId for that case.
//!
//! The big arrows represent [`rendering::common::types`] (IR Types) that should always have
//! [`rend3::types::ResourceHandle`]s, but those could also be lazily loaded (i.e. the IR
//! representation is already loaded/converted, but not uploaded to the driver yet).
//!
//! It's important to note that thin arrows should be built immediately and kept (unless above
//! optimizations kick in), but the big arrows are what is under the control of the renderer backend.
//! The big arrows can be dropped and built any time, especially the second layer (as the first
//! layer requires invoking the [`rendering::loader`] on the file represented by the node, but
//! could still happen, especially for M2 LoDs, where the per-lod mesh data resides in a separate file).
//!
//! Transforms/object handles are managed by having each reference to a mesh like (M2 reference,
//! WMO placement, ADT internal terrain meshes) having an additional transform/matrix3A that will
//! construct a [`rend3::types::ObjectHandle`] on the GPU side.
//!
//!
pub mod m2_generator;
pub mod nodes;
pub mod resolver;
