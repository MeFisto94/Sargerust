use std::ops::{Deref, DerefMut};
use std::sync::{Arc, OnceLock, RwLock, Weak};

use crate::game::application::GameApplication;
use crate::physics::character_movement_information::CharacterMovementInformation;
use crate::physics::collider_factory::ColliderFactory;
use crate::physics::physics_simulator::PhysicsSimulator;
use crate::physics::terrain_tile_colliders::{DoodadColliderEntry, TerrainTileColliders};
use crate::rendering::asset_graph::nodes::adt_node::{ADTNode, WMOGroupNode, WMONode};
use glam::{Vec3, Vec3A};
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
            ColliderFactory::process_terrain_tiles(
                &mut self.adt_nodes,
                &mut self.physics_simulator,
                handle,
                adt,
            );

            ColliderFactory::process_wmos(
                &mut self.wmo_colliders,
                &mut self.wmo_doodads,
                &mut self.physics_simulator,
                handle,
                adt,
            );
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
