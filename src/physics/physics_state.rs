use std::ops::{Deref, DerefMut};
use std::sync::{Arc, Mutex, OnceLock, Weak};

use glam::{Vec3, Vec3A};
use itertools::Itertools;
use rapier3d::control::KinematicCharacterController;
use rapier3d::prelude::*;

use crate::game::application::GameApplication;
use crate::physics::character_movement_information::CharacterMovementInformation;
use crate::physics::physics_simulator::PhysicsSimulator;
use crate::physics::terrain_tile_colliders::TerrainTileColliders;
use crate::rendering::asset_graph::nodes::adt_node::{
    ADTNode, DoodadReference, IRMesh, M2Node, NodeReference, TerrainTile, WMOGroupNode, WMONode,
};
use crate::rendering::common::coordinate_systems;

pub struct PhysicsState {
    app: Weak<GameApplication>,
    physics_simulator: PhysicsSimulator,
    rigid_body_handle: OnceLock<RigidBodyHandle>,
    character_controller: KinematicCharacterController,
    character_controller_collider: Option<ColliderHandle>,
    adt_nodes: Vec<(Weak<ADTNode>, TerrainTileColliders)>,
    wmo_doodads: Vec<(
        Weak<WMONode>,
        Arc<Mutex<Vec<(Weak<M2Node>, ColliderHandle)>>>,
    )>,
    wmo_colliders: Vec<(
        Weak<WMONode>,
        Arc<Mutex<Vec<(Weak<WMOGroupNode>, ColliderHandle)>>>,
    )>,
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
                ..KinematicCharacterController::default()
            },
            character_controller_collider: None,
            wmo_doodads: vec![],
            wmo_colliders: vec![],
        }
    }

    fn app(&self) -> Arc<GameApplication> {
        self.app.upgrade().expect("Weak Pointer expired")
    }

    pub fn update_fixed(&mut self, movement_relative: Vec3) -> CharacterMovementInformation {
        if self.character_controller_collider.is_none() {
            self.character_controller_collider = Some(self.create_character_collider());
        }

        let collider = self
            .character_controller_collider
            .expect("has to be constructed already");

        self.delta_map();
        let char = self.update_character(collider, movement_relative, false);
        self.physics_simulator.step();
        char
    }

    // TODO: Implement notifications via https://docs.rs/tokio/latest/tokio/sync/broadcast/index.html

    // TODO: Collider with heightfield at low res or rather meshes? Mesh would have the benefit of
    //  already being in adt/whatever space. In the end, it should be a heightfield for performance reasons, though.
    pub fn delta_map(&mut self) {
        // Find changed (i.e. added or removed) tiles. Currently we don't go after interior changes.
        let app = self.app();
        let mm_lock = app.game_state.clone().map_manager.clone();
        let mm = mm_lock.read().expect("Read Lock on Map Manager");
        let handle = self.terrain_rb();

        for (coords, adt) in &mm.tile_graph {
            let weak = Arc::downgrade(adt);
            if !self.adt_nodes.iter().any(|entry| entry.0.ptr_eq(&weak)) {
                let colliders = adt.terrain.iter().map(|terrain| terrain.into()).collect();
                let collider_handles = self.physics_simulator.insert_colliders(colliders, handle);
                self.adt_nodes
                    .push((weak.clone(), TerrainTileColliders::new(collider_handles)));
            }

            // At this point, there has to be a value within adt_nodes that has the terrain colliders
            let colliders = self
                .adt_nodes
                .iter_mut()
                .find(|entry| entry.0.ptr_eq(&weak))
                .expect("Logical error")
                .1
                .doodad_colliders
                .clone();

            self.process_direct_doodads(handle, &adt.doodads, colliders);
            self.process_wmo_doodads(handle, adt);
            self.process_wmos(handle, adt);
        }

        for (weak, tile_colliders) in self.adt_nodes.deref() {
            if weak.upgrade().is_none() {
                for &collider in &tile_colliders.terrain_colliders {
                    self.physics_simulator.drop_collider(collider, false);
                }

                // TODO: Drop all other colliders and remove entries.
            }
        }
    }

    fn process_wmos(&mut self, handle: RigidBodyHandle, adt: &Arc<ADTNode>) {
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

                let wmo_root_translation: Vec3A = wmo_ref.transform.translation;
                self.process_wmo_groups(handle, wmo_root_translation, &wmo.subgroups, colliders);
            }
        }
    }

    // TODO: maybe make this method generic.
    fn process_wmo_groups(
        &mut self,
        handle: RigidBodyHandle,
        wmo_root_translation: Vec3A,
        groups: &Vec<Arc<NodeReference<WMOGroupNode>>>,
        colliders: Arc<Mutex<Vec<(Weak<WMOGroupNode>, ColliderHandle)>>>,
    ) {
        for group_reference in groups {
            let resolved_group = group_reference.reference.read().expect("poisoned lock");
            if let Some(group) = resolved_group.deref() {
                let weak_grp = Arc::downgrade(group);

                let has_collider_for_key = colliders
                    .lock()
                    .expect("poisoned lock")
                    .iter()
                    .any(|dac| dac.0.ptr_eq(&weak_grp));

                if !has_collider_for_key {
                    // Technically, these aren't multiple batches, just multiple LoD levels.
                    let mesh_lock = group.mesh_batches.first().expect("at least one LoD level");
                    let mesh = mesh_lock.read().expect("poisoned lock");

                    // TODO: maybe this also needs to be done for the actual rotation?
                    // We need to counter convert because physics seem to have a different coordinate system.
                    let doodad_position: Vec3 = coordinate_systems::blender_to_adt(wmo_root_translation).into();

                    let mut doodad_collider: Collider = mesh.deref().into();
                    doodad_collider.set_position(doodad_position.into());

                    // TODO: This is questionable. Should all doodads be their own, but fixed, rigid body or part
                    //  of the terrain body. The latter sounds more reasonable for most cases, provided the physics
                    //  engine can handle that.
                    let doodad_handle: ColliderHandle = *self
                        .physics_simulator
                        .insert_colliders(vec![doodad_collider], handle)
                        .first()
                        .unwrap();

                    colliders
                        .lock()
                        .expect("poisoned lock")
                        .push((weak_grp, doodad_handle));

                    log::trace!(
                        "Adding collider for WMO Group {}",
                        group_reference.reference_str
                    );
                }
            }
        }
    }

    fn process_wmo_doodads(&mut self, handle: RigidBodyHandle, adt: &Arc<ADTNode>) {
        for wmo_ref in &adt.wmos {
            let resolved_wmo = wmo_ref.reference.reference.read().expect("poisoned lock");
            if let Some(wmo) = resolved_wmo.deref() {
                let weak_wmo = Arc::downgrade(wmo);

                if !self
                    .wmo_doodads
                    .iter_mut()
                    .any(|entry| entry.0.ptr_eq(&weak_wmo))
                {
                    // No collider for that wmo yet, but we have resolved the reference, so we can submit collision handles.
                    self.wmo_doodads
                        .push((weak_wmo.clone(), Default::default()));
                }

                let colliders = self
                    .wmo_doodads
                    .iter_mut()
                    .find(|entry| entry.0.ptr_eq(&weak_wmo))
                    .expect("Logical error")
                    .1
                    .clone();

                self.process_direct_doodads(handle, &wmo.doodads, colliders);
            }
        }
    }

    fn process_direct_doodads(
        &mut self,
        handle: RigidBodyHandle,
        doodads: &Vec<Arc<DoodadReference>>,
        doodad_colliders: Arc<Mutex<Vec<(Weak<M2Node>, ColliderHandle)>>>,
    ) {
        for doodad in doodads {
            let resolved_doodad = doodad.reference.reference.read().expect("poisoned lock");
            if let Some(dad) = resolved_doodad.deref() {
                let weak_dad = Arc::downgrade(dad);

                let has_collider_for_key = doodad_colliders
                    .lock()
                    .expect("poisoned lock")
                    .iter()
                    .any(|dac| dac.0.ptr_eq(&weak_dad));

                if !has_collider_for_key {
                    // doodad has been resolved and hasn't been added to the physics scene yet
                    // We need to counter convert because physics seem to have a different coordinate system.
                    // TODO: rotation too?
                    let doodad_position: Vec3 =
                        coordinate_systems::blender_to_adt(doodad.transform.to_scale_rotation_translation().2.into())
                            .into();

                    let mut doodad_collider: Collider = dad.deref().into();
                    doodad_collider.set_position(doodad_position.into());

                    // TODO: This is questionable. Should all doodads be their own, but fixed, rigid body or part
                    //  of the terrain body. The latter sounds more reasonable for most cases, provided the physics
                    //  engine can handle that.
                    let doodad_handle: ColliderHandle = *self
                        .physics_simulator
                        .insert_colliders(vec![doodad_collider], handle)
                        .first()
                        .unwrap();

                    doodad_colliders
                        .lock()
                        .expect("poisoned lock")
                        .push((weak_dad, doodad_handle));

                    log::trace!(
                        "Adding Doodad collider for {}",
                        doodad.reference.reference_str
                    );
                }
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
        pos_vec3.z += 3.0; // compare this in update_character for the reasoning.

        let coll = ColliderBuilder::capsule_z(3.0, 0.5)
            .position(pos_vec3.into())
            .build();
        self.physics_simulator.insert_collider(coll)
    }

    pub fn update_character(
        &mut self,
        collider: ColliderHandle,
        movement_relative: Vec3,
        flying: bool,
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
        pos.z += 3.0;
        pos.z += 0.2 * 3.0; // offset

        // Update the collider first
        self.physics_simulator.teleport_collider(collider, pos);

        // TODO: when not flying, we could also null z-axis forces in movement_relative.

        let gravity = if !flying {
            Vec3::new(0.0, 0.0, -9.81 * 1.0 / 60.0)
        } else {
            Vec3::ZERO
        };

        let movement = self.physics_simulator.move_character(
            &self.character_controller,
            collider,
            50.0,
            movement_relative + gravity,
        );

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
        let vertices = value
            .data
            .vertex_buffers
            .position_buffer
            .iter()
            .map(|&vert| vert.into())
            .collect();

        let indices = value
            .data
            .index_buffer
            .clone()
            .into_iter()
            .array_chunks()
            .collect();

        ColliderBuilder::trimesh(vertices, indices).build()
    }
}
