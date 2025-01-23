use crate::physics::physics_simulator::PhysicsSimulator;
use crate::physics::terrain_tile_colliders::{DoodadColliderEntry, TerrainTileColliders};
use crate::rendering::asset_graph::nodes::adt_node::{
    ADTNode, DoodadReference, IRMesh, M2Node, NodeReference, TerrainTile, WMOGroupNode, WMONode, WMOReference,
};
use crate::rendering::common::coordinate_systems;
use crate::rendering::common::mesh_merger::MeshMerger;
use crate::rendering::common::types::Mesh;
use glam::{Affine3A, Quat, Vec3};
use itertools::Itertools;
use log::trace;
use nalgebra::Isometry3;
use rapier3d::dynamics::RigidBodyHandle;
use rapier3d::geometry::{Collider, ColliderBuilder, ColliderHandle, MeshConverter};
use std::ops::Deref;
use std::sync::{Arc, RwLock, Weak};

pub enum ColliderFactory {}

impl ColliderFactory {
    pub fn process_terrain_tiles(
        adt_nodes: &mut Vec<(Weak<ADTNode>, TerrainTileColliders)>,
        simulator: &mut PhysicsSimulator,
        handle: RigidBodyHandle,
        adt: &Arc<ADTNode>,
    ) {
        let weak = Arc::downgrade(adt);
        Self::process_terrain_heightmap(adt_nodes, simulator, handle, adt, &weak);

        // At this point, there has to be a value within adt_nodes that has the terrain colliders
        let doodad_colliders = adt_nodes
            .iter()
            .find(|entry| entry.0.ptr_eq(&weak))
            .expect("Logical error")
            .1
            .doodad_colliders
            .clone();

        Self::process_doodads(simulator, handle, &adt.doodads, &doodad_colliders, None);
    }

    fn process_terrain_heightmap(
        adt_nodes: &mut Vec<(Weak<ADTNode>, TerrainTileColliders)>,
        simulator: &mut PhysicsSimulator,
        handle: RigidBodyHandle,
        adt: &Arc<ADTNode>,
        weak: &Weak<ADTNode>,
    ) {
        if !adt_nodes.iter().any(|entry| entry.0.ptr_eq(weak)) {
            let colliders = adt.terrain.iter().map(|terrain| terrain.into()).collect();
            let collider_handles = simulator.insert_colliders(colliders, handle);
            adt_nodes.push((weak.clone(), TerrainTileColliders::new(collider_handles)));
        }
    }

    pub fn process_wmos(
        wmo_colliders: &mut Vec<(
            Weak<WMONode>,
            Arc<RwLock<Vec<(Weak<WMOGroupNode>, ColliderHandle)>>>,
        )>,
        wmo_doodads: &mut Vec<(Weak<WMONode>, Arc<RwLock<Vec<DoodadColliderEntry>>>)>,
        simulator: &mut PhysicsSimulator,
        handle: RigidBodyHandle,
        adt: &ADTNode,
    ) {
        for wmo_ref in &adt.wmos {
            let resolved_wmo = wmo_ref.reference.reference.read().expect("poisoned lock");
            if let Some(wmo) = resolved_wmo.deref() {
                let weak_wmo = Arc::downgrade(wmo);

                if !wmo_colliders
                    .iter_mut()
                    .any(|entry| entry.0.ptr_eq(&weak_wmo))
                {
                    wmo_colliders.push((weak_wmo.clone(), Default::default()));
                }

                let colliders = wmo_colliders
                    .iter_mut()
                    .find(|entry| entry.0.ptr_eq(&weak_wmo))
                    .expect("Logical error")
                    .1
                    .clone();

                Self::process_wmo_groups(simulator, handle, wmo_ref, &wmo.subgroups, &colliders);
                Self::process_wmo_doodads(wmo_doodads, simulator, handle, &weak_wmo, wmo_ref, wmo);
            }
        }
    }

    fn process_wmo_groups(
        simulator: &mut PhysicsSimulator,
        handle: RigidBodyHandle,
        wmo_ref: &WMOReference,
        groups: &Vec<Arc<NodeReference<WMOGroupNode>>>,
        colliders: &RwLock<Vec<(Weak<WMOGroupNode>, ColliderHandle)>>,
    ) {
        let (scale, rotation, translation) = wmo_ref.transform.to_scale_rotation_translation();
        for group_reference in groups {
            let resolved_group = group_reference.reference.read().expect("poisoned lock");
            if let Some(group) = resolved_group.deref() {
                let weak_grp = Arc::downgrade(group);

                {
                    let has_collider_for_key = colliders
                        .read()
                        .expect("poisoned read lock")
                        .iter()
                        .any(|entry| entry.0.ptr_eq(&weak_grp));

                    if has_collider_for_key {
                        continue;
                    }
                }

                trace!(
                    "Adding collider for WMO Group {}",
                    group_reference.reference_str
                );

                let mesh_batches = group
                    .mesh_batches
                    .iter()
                    // TODO: Get rid of that clone
                    .map(|mesh_lock| mesh_lock.read().expect("poisoned read lock").data.clone())
                    .collect_vec();
                let mut mesh = MeshMerger::merge_meshes_index_only(&mesh_batches);

                // TODO: Validate that the coordinate systems are matching, but since we are rotating the mesh
                //  afterwards, I think for now mesh and scale are in the same coordinate system
                MeshMerger::mesh_scale_position(&mut mesh, scale);

                // We need to counter convert because physics seem to have a different coordinate system.
                let conversion_quat = Quat::from_mat4(&coordinate_systems::blender_to_adt_rot());
                let wmo_translation: Vec3 = coordinate_systems::blender_to_adt(translation.into()).into();
                let wmo_rotation: Quat = conversion_quat.mul_quat(rotation);

                let mut wmo_collider: Collider = (&mesh).into();
                wmo_collider.set_position(Isometry3::from((wmo_translation, wmo_rotation)));

                let wmo_handle: ColliderHandle = *simulator
                    .insert_colliders(vec![wmo_collider], handle)
                    .first()
                    .expect("Insert Colliders exactly inserted one collider");

                {
                    colliders
                        .write()
                        .expect("poisoned write lock")
                        .push((weak_grp, wmo_handle));
                }
            }
        }
    }

    fn process_wmo_doodads(
        wmo_doodads: &mut Vec<(Weak<WMONode>, Arc<RwLock<Vec<DoodadColliderEntry>>>)>,
        simulator: &mut PhysicsSimulator,
        handle: RigidBodyHandle,
        weak_wmo: &Weak<WMONode>,
        wmo_ref: &WMOReference,
        wmo: &WMONode,
    ) {
        if !wmo_doodads.iter_mut().any(|entry| entry.0.ptr_eq(weak_wmo)) {
            // No collider for that wmo yet, but we have resolved the reference, so we can submit collision handles.
            wmo_doodads.push((weak_wmo.clone(), Default::default()));
        }

        let colliders = wmo_doodads
            .iter_mut()
            .find(|entry| entry.0.ptr_eq(weak_wmo))
            .expect("Logical error")
            .1
            .clone();

        Self::process_doodads(
            simulator,
            handle,
            &wmo.doodads,
            &colliders,
            Some(wmo_ref.transform),
        );
    }

    fn process_doodads(
        simulator: &mut PhysicsSimulator,
        handle: RigidBodyHandle,
        doodads: &Vec<Arc<DoodadReference>>,
        doodad_colliders: &RwLock<Vec<DoodadColliderEntry>>,
        parent_transform: Option<Affine3A>,
    ) {
        // TODO: A lot of those doodads probably shouldn't be collidable (lilypad, jugs, wall shield)
        for doodad in doodads {
            let weak_doodad = Arc::downgrade(doodad);
            let resolved_doodad = doodad.reference.reference.read().expect("poisoned lock");

            let Some(dad) = resolved_doodad.deref() else {
                continue;
            };

            {
                let has_collider_for_key = doodad_colliders
                    .read()
                    .expect("poisoned lock")
                    .iter()
                    .any(|entry| entry.reference.ptr_eq(&weak_doodad));

                if has_collider_for_key {
                    continue;
                }
            }

            // doodad has been resolved but hasn't been added to the physics scene, yet
            // We need to counter convert coordinate systems, because physics seem to have yet a different coordinate system.

            let transform = parent_transform
                .map(|parent| parent * doodad.transform)
                .unwrap_or(doodad.transform);

            let (scale, rotation, translation) = transform.to_scale_rotation_translation();
            let conversion_quat = Quat::from_mat4(&coordinate_systems::blender_to_adt_rot());
            let doodad_translation: Vec3 = coordinate_systems::blender_to_adt(translation.into()).into();
            let doodad_rotation: Quat = conversion_quat.mul_quat(rotation);

            trace!(
                "Adding Doodad collider for {} at {}",
                doodad.reference.reference_str, doodad_translation
            );

            // TODO: I have the feeling the colliders don't work as before anymore, but maybe I am mistaken.
            let meshes: Vec<Mesh> = dad
                .meshes_and_materials
                .iter()
                .map(|(mesh, _)| mesh.read().expect("Mesh Read Lock").data.clone())
                .collect();
            let mut mesh = MeshMerger::merge_meshes_vertices_only(&meshes);

            // TODO: Validate that the coordinate systems are matching, but since we are rotating the mesh
            //  afterwards, I think for now mesh and scale are in the same coordinate system
            MeshMerger::mesh_scale_position(&mut mesh, scale);

            let mut doodad_collider: Collider = (&mesh).into();
            doodad_collider.set_position(Isometry3::from((doodad_translation, doodad_rotation)));

            // TODO: This is questionable. Should all doodads be their own, but fixed, rigid body or part
            //  of the terrain body. The latter sounds more reasonable for most cases, provided the physics
            //  engine can handle that.
            let doodad_handle: ColliderHandle = *simulator
                .insert_colliders(vec![doodad_collider], handle)
                .first()
                .expect("Insert Colliders exactly inserted one collider");

            {
                doodad_colliders
                    .write()
                    .expect("write lock poisoned")
                    .push(DoodadColliderEntry {
                        reference: weak_doodad,
                        collider_handle: doodad_handle,
                    });
            }
        }
    }
}

// TODO: We have differing implementations of From<T> for Collider. Some set the position, some don't
impl From<&TerrainTile> for Collider {
    fn from(value: &TerrainTile) -> Self {
        let mut collider: Collider = value.mesh.read().expect("Mesh RLock").deref().into();
        collider.set_position(Into::<Vec3>::into(value.position).into());
        collider
    }
}

impl From<&M2Node> for Collider {
    fn from(value: &M2Node) -> Self {
        // We opted for silently merging, because there won't be a reason to treat submeshes as their own collider. They
        // only exist because they have different materials

        let meshes: Vec<Mesh> = value
            .meshes_and_materials
            .iter()
            .map(|(mesh, _)| mesh.read().expect("Mesh Read Lock").data.clone())
            .collect();

        (&MeshMerger::merge_meshes_vertices_only(&meshes)).into()
    }
}

impl From<&IRMesh> for Collider {
    fn from(value: &IRMesh) -> Self {
        (&value.data).into()
    }
}

impl From<&Mesh> for Collider {
    fn from(value: &Mesh) -> Self {
        let vertices = value
            .vertex_buffers
            .position_buffer
            .iter()
            .map(|&vert| vert.into())
            .collect();

        let indices = value
            .index_buffer
            .clone()
            .into_iter()
            .array_chunks()
            .collect();

        // TODO: depending on settings, we may as well use convex hull / convex decomposition, but convex hull had bad (unpredictable) results and convex decomposition took a *long* time
        //  to the point where using the trimesh and the building the convex decomposition in the background may be required.
        let converter = MeshConverter::TriMesh;

        ColliderBuilder::converted_trimesh(vertices, indices, converter)
            .expect("Valid IRMesh Collider Builder")
            .build()
    }
}
