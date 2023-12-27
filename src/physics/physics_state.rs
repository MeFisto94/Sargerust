use crate::game::application::GameApplication;
use crate::physics::character_movement_information::CharacterMovementInformation;
use crate::physics::physics_simulator::PhysicsSimulator;
use crate::physics::terrain_tile_colliders::TerrainTileColliders;
use crate::rendering::asset_graph::nodes::adt_node::{ADTNode, DoodadReference, M2Node, TerrainTile, WMONode};
use crate::rendering::common::coordinate_systems;
use glam::{Vec3, Vec3A};
use itertools::Itertools;
use rapier3d::control::KinematicCharacterController;
use rapier3d::prelude::*;
use std::ops::{Deref, DerefMut};
use std::sync::{Arc, OnceLock, Weak};

pub struct PhysicsState {
    app: Weak<GameApplication>,
    physics_simulator: PhysicsSimulator,
    rigid_body_handle: OnceLock<RigidBodyHandle>,
    character_controller: KinematicCharacterController,
    character_controller_collider: Option<ColliderHandle>,
    adt_nodes: Vec<(Weak<ADTNode>, TerrainTileColliders)>,
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

                // TODO: add wmo doodad colliders
                // TODO: add wmo colliders
            }

            // At this point, there has to be a value within adt_nodes that has the terrain colliders
            // TODO: maybe some rust magic (e.g. interior mutability?) would allow us to pull out "nodes"
            self.process_direct_doodads(handle, adt, &weak);
        }

        for (weak, tile_colliders) in &self.adt_nodes {
            if weak.upgrade().is_none() {
                for &collider in &tile_colliders.terrain_colliders {
                    self.physics_simulator.drop_collider(collider, false);
                }

                // TODO: Remove entry
                // TODO: drop wmo colliders
            }
        }
    }

    fn process_direct_doodads(&mut self, handle: RigidBodyHandle, adt: &Arc<ADTNode>, weak: &Weak<ADTNode>) {
        let nodes = self
            .adt_nodes
            .iter_mut()
            .find(|entry| entry.0.ptr_eq(weak))
            .expect("Logical error");

        for doodad in &adt.doodads {
            let resolved_doodad = doodad.reference.reference.read().expect("poisoned lock");
            if let Some(dad) = resolved_doodad.deref() {
                let weak_dad = Arc::downgrade(dad);
                if !nodes
                    .1
                    .doodad_colliders
                    .iter()
                    .any(|dac| dac.0.ptr_eq(&weak_dad))
                {
                    // doodad has been resolved and hasn't been added to the physics scene yet

                    // We need to counter convert because physics seem to have a different coordinate system.
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
                    nodes.1.doodad_colliders.push((weak_dad, doodad_handle));
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

impl From<&TerrainTile> for Collider {
    fn from(value: &TerrainTile) -> Self {
        let meshlock = value.mesh.read().expect("Mesh RLock");
        let vertices = meshlock
            .data
            .vertex_buffers
            .position_buffer
            .iter()
            .map(|&vert| vert.into())
            .collect();

        let indices = meshlock
            .data
            .index_buffer
            .clone()
            .into_iter()
            .array_chunks()
            .collect();

        ColliderBuilder::trimesh(vertices, indices)
            .position(Into::<Vec3>::into(value.position).into())
            .build()
    }
}

// TODO: We have differing implementations of From<T> for Collider. Some set the position, some don't
impl From<&M2Node> for Collider {
    fn from(value: &M2Node) -> Self {
        let meshlock = value.mesh.read().expect("Mesh RLock");
        let vertices = meshlock
            .data
            .vertex_buffers
            .position_buffer
            .iter()
            .map(|&vert| vert.into())
            .collect();

        let indices = meshlock
            .data
            .index_buffer
            .clone()
            .into_iter()
            .array_chunks()
            .collect();

        ColliderBuilder::trimesh(vertices, indices).build()
    }
}
