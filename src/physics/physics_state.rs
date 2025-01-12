use std::ops::{Deref, DerefMut};
use std::sync::{Arc, OnceLock, RwLock, Weak};

use crate::game::application::GameApplication;
use crate::physics::character_movement_information::CharacterMovementInformation;
use crate::physics::physics_simulator::PhysicsSimulator;
use crate::physics::terrain_tile_colliders::{DoodadColliderEntry, TerrainTileColliders};
use crate::rendering::asset_graph::nodes::adt_node::{
    ADTNode, DoodadReference, IRMesh, M2Node, NodeReference, TerrainTile, WMOGroupNode, WMONode, WMOReference,
};
use crate::rendering::common::coordinate_systems;
use crate::rendering::common::mesh_merger::MeshMerger;
use crate::rendering::common::types::Mesh;
use glam::{Affine3A, Quat, Vec3, Vec3A};
use itertools::Itertools;
use log::trace;
use nalgebra::Isometry3;
use rapier3d::control::KinematicCharacterController;
use rapier3d::prelude::*;

pub struct PhysicsState {
    app: Weak<GameApplication>,
    physics_simulator: PhysicsSimulator,
    rigid_body_handle: OnceLock<RigidBodyHandle>,
    character_controller: KinematicCharacterController,
    character_controller_collider: Option<ColliderHandle>, // TODO: We could get rid of the Option and just create a collider at (0, 0, 0), we'll teleport it every frame anyway.
    adt_nodes: Vec<(Weak<ADTNode>, TerrainTileColliders)>,
    wmo_doodads: Vec<(Weak<WMONode>, Arc<RwLock<Vec<DoodadColliderEntry>>>)>,
    wmo_colliders: Vec<(
        Weak<WMONode>,
        Arc<RwLock<Vec<(Weak<WMOGroupNode>, ColliderHandle)>>>,
    )>,
    time_since_airborne: f32,
}

impl PhysicsState {
    pub fn new(app: Weak<GameApplication>) -> Self {
        Self {
            app,
            physics_simulator: PhysicsSimulator::default(),
            adt_nodes: vec![],
            rigid_body_handle: OnceLock::new(),
            character_controller: KinematicCharacterController {
                up: Vector::z_axis(),
                max_slope_climb_angle: std::f32::consts::FRAC_PI_2,
                min_slope_slide_angle: std::f32::consts::FRAC_PI_4,
                normal_nudge_factor: 5.0e-2, // was e-4. bigger value -> faster ascent, too large: jitter
                slide: true,
                //snap_to_ground: Some(CharacterLength::Relative(1.0)),
                // autostep: Some(CharacterAutostep {
                //     include_dynamic_bodies: false,
                //     max_height: CharacterLength::Relative(3.75),
                //     min_width: CharacterLength::Relative(0.001),
                // }),
                ..KinematicCharacterController::default()
            },
            character_controller_collider: None,
            wmo_doodads: vec![],
            wmo_colliders: vec![],
            time_since_airborne: 0.0,
        }
    }

    fn app(&self) -> Arc<GameApplication> {
        self.app.upgrade().expect("Weak Pointer expired")
    }

    pub fn update_fixed(&mut self, movement_relative: Vec3) -> CharacterMovementInformation {
        if self.character_controller_collider.is_none() {
            self.character_controller_collider = Some(self.create_character_collider());
        }

        let timestep = 1.0 / 60.0; // TODO: why does physics_simulator not have a timestep?
        let collider = self
            .character_controller_collider
            .expect("has to be constructed already");

        self.delta_map();
        let char = self.update_character(collider, movement_relative, false, timestep);
        self.physics_simulator.step();
        char
    }

    // TODO: Implement notifications via https://docs.rs/tokio/latest/tokio/sync/broadcast/index.html

    // TODO: Collider with heightfield at low res or rather meshes? Mesh would have the benefit of
    //  already being in adt/whatever space. In the end, it should be a heightfield for performance reasons, though.
    pub fn delta_map(&mut self) {
        // Find changed (i.e. added or removed) tiles. Currently, we don't go after interior changes.
        let app = self.app();
        let mm_lock = app.game_state.clone().map_manager.clone();
        let mm = mm_lock.read().expect("Read Lock on Map Manager");
        let handle = self.terrain_rb();

        for adt in mm.tile_graph.values() {
            self.process_terrain_tiles(handle, adt);
            self.process_wmos(handle, adt);
        }

        for (weak, tile_colliders) in self.adt_nodes.deref() {
            if weak.upgrade().is_none() {
                for &collider in &tile_colliders.terrain_colliders {
                    self.physics_simulator.drop_collider(collider, false);
                }

                let doodad_colliders = tile_colliders
                    .doodad_colliders
                    .read()
                    .expect("poisoned lock");

                for collider in doodad_colliders.iter() {
                    self.physics_simulator
                        .drop_collider(collider.collider_handle, false);
                }

                // TODO: Drop all other colliders and remove entries.
            } // TODO: Else - technically some of the weaks below may be none, it's rather hypothetical, though.
        }
    }

    fn process_terrain_tiles(&mut self, handle: RigidBodyHandle, adt: &Arc<ADTNode>) {
        let weak = Arc::downgrade(adt);
        self.process_terrain_heightmap(handle, adt, &weak);

        // At this point, there has to be a value within adt_nodes that has the terrain colliders
        let doodad_colliders = self
            .adt_nodes
            .iter()
            .find(|entry| entry.0.ptr_eq(&weak))
            .expect("Logical error")
            .1
            .doodad_colliders
            .clone();

        self.process_doodads(handle, &adt.doodads, &doodad_colliders, None);
    }

    fn process_terrain_heightmap(&mut self, handle: RigidBodyHandle, adt: &Arc<ADTNode>, weak: &Weak<ADTNode>) {
        if !self.adt_nodes.iter().any(|entry| entry.0.ptr_eq(weak)) {
            let colliders = adt.terrain.iter().map(|terrain| terrain.into()).collect();
            let collider_handles = self.physics_simulator.insert_colliders(colliders, handle);
            self.adt_nodes
                .push((weak.clone(), TerrainTileColliders::new(collider_handles)));
        }
    }

    fn process_wmos(&mut self, handle: RigidBodyHandle, adt: &ADTNode) {
        for wmo_ref in &adt.wmos {
            let resolved_wmo = wmo_ref.reference.reference.read().expect("poisoned lock");
            if let Some(wmo) = resolved_wmo.deref() {
                let weak_wmo = Arc::downgrade(wmo);

                if !self
                    .wmo_colliders
                    .iter_mut()
                    .any(|entry| entry.0.ptr_eq(&weak_wmo))
                {
                    self.wmo_colliders
                        .push((weak_wmo.clone(), Default::default()));
                }

                let colliders = self
                    .wmo_colliders
                    .iter_mut()
                    .find(|entry| entry.0.ptr_eq(&weak_wmo))
                    .expect("Logical error")
                    .1
                    .clone();

                self.process_wmo_groups(handle, wmo_ref, &wmo.subgroups, &colliders);
                self.process_wmo_doodads(handle, &weak_wmo, wmo_ref, wmo);
            }
        }
    }

    fn process_wmo_groups(
        &mut self,
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

                // This is placed here, because that way it will log once per group, not once per frame!
                if !scale.abs_diff_eq(Vec3::ONE, 1e-3) {
                    log::warn!(
                        "WMO {} has non-unit scale ({}), which is not supported by the physics engine, yet",
                        wmo_ref.reference.reference_str,
                        scale
                    );
                }

                let mesh_batches = group
                    .mesh_batches
                    .iter()
                    // TODO: Get rid of that clone
                    .map(|mesh_lock| mesh_lock.read().expect("poisoned read lock").data.clone())
                    .collect_vec();
                let mesh = MeshMerger::merge_meshes_index_only(&mesh_batches);

                // We need to counter convert because physics seem to have a different coordinate system.
                let conversion_quat = Quat::from_mat4(&coordinate_systems::blender_to_adt_rot());
                let wmo_translation: Vec3 = coordinate_systems::blender_to_adt(translation.into()).into();
                let wmo_rotation: Quat = conversion_quat.mul_quat(rotation);

                let mut wmo_collider: Collider = (&mesh).into();
                wmo_collider.set_position(Isometry3::from((wmo_translation, wmo_rotation)));

                let wmo_handle: ColliderHandle = *self
                    .physics_simulator
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
        &mut self,
        handle: RigidBodyHandle,
        weak_wmo: &Weak<WMONode>,
        wmo_ref: &WMOReference,
        wmo: &WMONode,
    ) {
        if !self
            .wmo_doodads
            .iter_mut()
            .any(|entry| entry.0.ptr_eq(weak_wmo))
        {
            // No collider for that wmo yet, but we have resolved the reference, so we can submit collision handles.
            self.wmo_doodads
                .push((weak_wmo.clone(), Default::default()));
        }

        let colliders = self
            .wmo_doodads
            .iter_mut()
            .find(|entry| entry.0.ptr_eq(weak_wmo))
            .expect("Logical error")
            .1
            .clone();

        self.process_doodads(handle, &wmo.doodads, &colliders, Some(wmo_ref.transform));
    }

    fn process_doodads(
        &mut self,
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

            if !scale.abs_diff_eq(Vec3::ONE, 1e-3) {
                log::warn!(
                    "Doodad {} has non-unit scale ({}), which is not supported by the physics engine, yet",
                    doodad.reference.reference_str,
                    scale
                );
            }

            let mut doodad_collider: Collider = dad.deref().into();
            doodad_collider.set_position(Isometry3::from((doodad_translation, doodad_rotation)));

            // TODO: This is questionable. Should all doodads be their own, but fixed, rigid body or part
            //  of the terrain body. The latter sounds more reasonable for most cases, provided the physics
            //  engine can handle that.
            let doodad_handle: ColliderHandle = *self
                .physics_simulator
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

    fn terrain_rb(&mut self) -> RigidBodyHandle {
        *self.rigid_body_handle.get_or_init(|| {
            self.physics_simulator
                .insert_rigid_body(RigidBodyBuilder::fixed().build())
        })
    }

    fn create_character_collider(&mut self) -> ColliderHandle {
        let mut pos_vec3: Vec3 = {
            (*self
                .app()
                .game_state
                .player_location
                .read()
                .expect("player read lock"))
            .into()
        };
        pos_vec3.z += 1.0; // compare this in update_character for the reasoning.

        let coll = ColliderBuilder::capsule_z(1.0, 0.5)
            .position(pos_vec3.into())
            .build();
        self.physics_simulator.insert_collider(coll)
    }

    pub fn update_character(
        &mut self,
        collider: ColliderHandle,
        movement_relative: Vec3,
        flying: bool,
        timestep: f32,
    ) -> CharacterMovementInformation {
        // I think rapier does not care about the collider at all, all that is important is that it's a shape with a position
        let mut pos: Vec3 = {
            (*self
                .app()
                .game_state
                .player_location
                .read()
                .expect("player read lock"))
            .into()
        };

        // I think this is because of the capsule shape and considering the physics position to be the center?
        pos.z += 2.0;

        // Update the collider first
        self.physics_simulator.teleport_collider(collider, pos);

        // TODO: when not flying, we should also null z-axis forces in movement_relative, but we keep it for debugging at the moment.

        let mut movement = self.physics_simulator.move_character(
            &self.character_controller,
            collider,
            50.0,
            movement_relative,
        );

        if !flying && !movement.grounded {
            self.time_since_airborne += timestep;

            let sliding_movement = movement.translation;
            self.physics_simulator
                .teleport_collider(collider, pos + Vec3::from(sliding_movement)); // apply the previous movement

            if self.time_since_airborne >= 4.0 * timestep {
                let gravity_velocity = -9.81 * self.time_since_airborne * 0.5; /* TODO: Find the actual gravity that is good for the game-sense */
                movement = self.physics_simulator.move_character(
                    &self.character_controller,
                    collider,
                    50.0,
                    Vec3::new(0.0, 0.0, gravity_velocity * timestep),
                );

                movement.translation += sliding_movement;
            }
        } else {
            self.time_since_airborne = 0.0;
        }

        // TODO: actually, the absolute position is a bit too high, causing flying. Is this the capsule offset?

        let transl: Vec3A = movement.translation.into();
        let absolute_position = {
            let app = self.app();
            let mut wlock = app
                .game_state
                .player_location
                .write()
                .expect("player write lock");
            *wlock.deref_mut() += transl;
            *wlock
        };

        // TODO: Does this really belong inside the _physics_ state? I think it's here because
        //  technically we should honor the orientation at some point and constrain angular movement etc
        let orientation = {
            *self
                .app()
                .game_state
                .player_orientation
                .read()
                .expect("player orientation read lock")
        };

        CharacterMovementInformation {
            absolute_position: absolute_position.into(),
            orientation,
            delta_movement: transl.into(),
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
        value.mesh.read().expect("Mesh RLock").deref().into()
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
