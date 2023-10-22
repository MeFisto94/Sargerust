use crate::game::application::GameApplication;
use crate::physics::physics_simulator::PhysicsSimulator;
use crate::rendering::asset_graph::nodes::adt_node::{ADTNode, TerrainTile};
use glam::{Vec3, Vec3A};
use rapier3d::control::KinematicCharacterController;
use rapier3d::prelude::*;
use std::ops::DerefMut;
use std::sync::{Arc, Weak};

pub struct PhysicsState {
    app: Weak<GameApplication>,
    physics_simulator: PhysicsSimulator,
    rigid_body_handle: Option<RigidBodyHandle>,
    character_controller: KinematicCharacterController,
    character_controller_collider: Option<ColliderHandle>,
    adt_nodes: Vec<(Weak<ADTNode>, Vec<ColliderHandle>)>,
}

impl PhysicsState {
    pub fn new(app: Weak<GameApplication>) -> Self {
        Self {
            app,
            physics_simulator: PhysicsSimulator::default(),
            adt_nodes: vec![],
            rigid_body_handle: None,
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

    pub fn update_fixed(&mut self, movement_relative: Vec3) {
        if self.character_controller_collider.is_none() {
            let mut pos_vec3: Vec3 = {
                (*self
                    .app()
                    .game_state
                    .player_location
                    .read()
                    .expect("player read lock"))
                .into()
            };

            pos_vec3.z += 5.0;
            let coll = ColliderBuilder::capsule_z(3.0, 0.5)
                .position(pos_vec3.into())
                .build();
            self.character_controller_collider = Some(self.physics_simulator.insert_collider(coll));
        }

        if self.rigid_body_handle.is_none() {
            self.rigid_body_handle = Some(
                self.physics_simulator
                    .insert_rigid_body(RigidBodyBuilder::fixed().build()),
            )
        }

        self.delta_map();
        self.update_character(movement_relative, false);
        self.physics_simulator.step();
    }

    // TODO: Collider with heightfield at low res or rather meshes? Mesh would have the benefit of
    //  already being in adt/whatever space. In the end, it should be a heightfield for performance reasons, though.
    pub fn delta_map(&mut self) {
        // Find changed (i.e. added or removed) tiles. Currently we don't go after interior changes.
        let app = self.app();
        let mm_lock = app.game_state.clone().map_manager.clone();
        let mm = mm_lock.read().expect("Read Lock on Map Manager");

        for (coords, adt) in &mm.tile_graph {
            let weak = Arc::downgrade(adt);
            if !self.adt_nodes.iter().any(|entry| entry.0.ptr_eq(&weak)) {
                let colliders = adt.terrain.iter().map(|terrain| terrain.into()).collect();
                let handle = self.rigid_body_handle.expect("Terrain Rigid Body present");
                let collider_handles = self.physics_simulator.insert_colliders(colliders, handle);
                self.adt_nodes.push((weak, collider_handles));
            }
        }

        for (weak, colliders) in &self.adt_nodes {
            if weak.upgrade().is_none() {
                for &collider in colliders {
                    self.physics_simulator.drop_collider(collider, false);
                }

                // TODO: Remove.
            }
        }
    }

    pub fn update_character(&mut self, movement_relative: Vec3, flying: bool) {
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

        // Update the collider first
        self.physics_simulator
            .teleport_collider(self.character_controller_collider.expect(""), pos);

        // TODO: when not flying, we could also null z-axis forces in movement_relative.

        let gravity = if !flying {
            Vec3::new(0.0, 0.0, -9.81 * 1.0 / 60.0)
        } else {
            Vec3::ZERO
        };

        let movement = self.physics_simulator.move_character(
            &self.character_controller,
            self.character_controller_collider.expect(""),
            50.0,
            movement_relative + gravity,
        );

        {
            let app = self.app();
            let mut wlock = app
                .game_state
                .player_location
                .write()
                .expect("player write lock");
            let transl: Vec3A = movement.translation.into();
            *wlock.deref_mut() += transl;
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
