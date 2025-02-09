use crate::game::application::GameApplication;
use std::ops::{Deref, DerefMut};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, OnceLock, RwLock, Weak};
use std::time::Instant;
use crate::physics::character_movement_information::CharacterMovementInformation;
use crate::physics::collider_factory::ColliderFactory;
use crate::physics::physics_simulator::PhysicsSimulator;
use crate::physics::terrain_tile_colliders::TerrainTileColliders;
use crate::rendering::asset_graph::nodes::adt_node::{ADTNode, DoodadReference, WMOGroupNode, WMONode};
use crate::rendering::common::coordinate_systems;
use crate::util::weak_dashmap::WeakKeyDashMapPruneOnInsert;
use glam::{Vec3, Vec3A};
use log::warn;
use rapier3d::control::KinematicCharacterController;
use rapier3d::prelude::*;

#[derive(Default)]
struct MapData {
    pub adt_nodes: Mutex<Vec<(Weak<ADTNode>, TerrainTileColliders)>>,
    pub wmo_doodads: Mutex<
        Vec<(
            Weak<WMONode>,
            WeakKeyDashMapPruneOnInsert<DoodadReference, ColliderHandle>,
        )>,
    >,
    pub wmo_colliders: Mutex<
        Vec<(
            Weak<WMONode>,
            Arc<RwLock<Vec<(Weak<WMOGroupNode>, ColliderHandle)>>>,
        )>,
    >,
}

pub struct PhysicsState {
    app: Weak<GameApplication>,
    physics_simulator: RwLock<PhysicsSimulator>,
    rigid_body_handle: OnceLock<RigidBodyHandle>,
    character_controller: KinematicCharacterController,
    character_controller_collider: OnceLock<ColliderHandle>,
    map_data: MapData,
    time_since_airborne: Mutex<f32>,
    running: AtomicBool,
    delta_movement: Mutex<Vec3>,
}

impl PhysicsState {
    pub fn new(app: Weak<GameApplication>) -> Self {
        Self {
            app,
            physics_simulator: RwLock::new(PhysicsSimulator::default()),
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
            character_controller_collider: OnceLock::new(),
            map_data: MapData::default(),
            time_since_airborne: Mutex::new(0.0),
            running: AtomicBool::new(false),
            delta_movement: Mutex::new(Vec3::ZERO),
        }
    }

    fn app(&self) -> Arc<GameApplication> {
        self.app.upgrade().expect("Weak Pointer expired")
    }

    #[profiling::function]
    pub fn update_fixed(&self, movement_relative: Vec3) -> CharacterMovementInformation {
        let timestep = 1.0 / 60.0; // TODO: why does physics_simulator not have a timestep?

        self.delta_map();
        let char = self.update_character(
            self.character_collider(),
            movement_relative,
            false,
            timestep,
        );

        {
            self.physics_simulator
                .write()
                .expect("Simulator write lock")
                .step();
        }

        char
    }

    pub fn notify_delta_movement(&self, delta_movement: Vec3A) {
        let delta: Vec3 = coordinate_systems::blender_to_adt(delta_movement).into();
        let mut movement_lock = self.delta_movement.lock().expect("delta movement lock");
        *movement_lock += delta;
    }

    // TODO: Implement notifications via https://docs.rs/tokio/latest/tokio/sync/broadcast/index.html
    // TODO: Collider with heightfield at low res or rather meshes? Mesh would have the benefit of
    //  already being in adt/whatever space. In the end, it should be a heightfield for performance reasons, though.
    #[profiling::function]
    fn delta_map(&self) {
        // Find changed (i.e. added or removed) tiles. Currently, we don't go after interior changes.
        let app = self.app();
        let mm_lock = app.game_state.clone().map_manager.clone();
        let mm = mm_lock.read().expect("Read Lock on Map Manager");
        let handle = self.terrain_rb(); // Could write-lock simulator, so we have to wait until after.

        let mut physics_simulator = self
            .physics_simulator
            .write()
            .expect("Simulator Write Lock");

        // In the absence of partial borrows....
        let mut adt_nodes = self.map_data.adt_nodes.lock().expect("Map Data ADT Lock");
        let mut wmo = self
            .map_data
            .wmo_colliders
            .lock()
            .expect("Map Data WMO Lock");
        let mut wmo_doodad = self
            .map_data
            .wmo_doodads
            .lock()
            .expect("Map Data WMO DOODAD Lock");

        for adt in mm.tile_graph.values() {
            ColliderFactory::process_terrain_tiles(&mut adt_nodes, &mut physics_simulator, handle, adt);

            ColliderFactory::process_wmos(
                &mut wmo,
                &mut wmo_doodad,
                &mut physics_simulator,
                handle,
                adt,
            );
        }

        for (weak, tile_colliders) in adt_nodes.deref() {
            if weak.upgrade().is_none() {
                for &collider in &tile_colliders.terrain_colliders {
                    physics_simulator.drop_collider(collider, false);
                }

                let doodad_colliders = &tile_colliders.doodad_colliders;

                // TODO: such things have to be re-thought: The map will Drop collider handles if the DoodadReference
                //  is dropped. That may happen before this code is run, unless it's cross-referenced somewhere else.
                //  As such, we may need NotifyOnDrop<ColliderHandle>? Or what does the physics engine do on drop?
                //  maybe we would never need such an explicit removal as dropping a handle automatically drops it?
                for collider in doodad_colliders.values() {
                    physics_simulator.drop_collider(collider, false);
                }

                // TODO: Drop all other colliders and remove entries.
            } // TODO: Else - technically some of the weaks below may be none, it's rather hypothetical, though.
        }
    }

    fn terrain_rb(&self) -> RigidBodyHandle {
        *self.rigid_body_handle.get_or_init(|| {
            let mut simulator = self
                .physics_simulator
                .write()
                .expect("Simulator Write Lock");
            simulator.insert_rigid_body(RigidBodyBuilder::fixed().build())
        })
    }

    fn character_collider(&self) -> ColliderHandle {
        *self.character_controller_collider.get_or_init(|| {
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
            self.physics_simulator
                .write()
                .expect("Simulator write lock")
                .insert_collider(coll)
        })
    }

    #[profiling::function]
    fn update_character(
        &self,
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

        let movement = {
            let mut simulator = self
                .physics_simulator
                .write()
                .expect("Simulator write lock");

            // Update the collider first
            simulator.teleport_collider(collider, pos);

            // TODO: when not flying, we should also null z-axis forces in movement_relative, but we keep it for debugging at the moment.

            let mut movement = simulator.move_character(
                &self.character_controller,
                collider,
                50.0,
                movement_relative,
            );

            {
                let mut time_since_airborne = self
                    .time_since_airborne
                    .lock()
                    .expect("time_since_airborne");

                if !flying && !movement.grounded {
                    *time_since_airborne += timestep;

                    let sliding_movement = movement.translation;
                    simulator.teleport_collider(collider, pos + Vec3::from(sliding_movement)); // apply the previous movement

                    if *time_since_airborne >= 4.0 * timestep {
                        let gravity_velocity = -9.81 * *time_since_airborne * 0.5; /* TODO: Find the actual gravity that is good for the game-sense */
                        movement = simulator.move_character(
                            &self.character_controller,
                            collider,
                            50.0,
                            Vec3::new(0.0, 0.0, gravity_velocity * timestep),
                        );

                        movement.translation += sliding_movement;
                    }
                } else {
                    *time_since_airborne = 0.0;
                }
            }

            movement
        };

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

    // Threading
    pub fn start(this: Arc<PhysicsState>) -> std::thread::JoinHandle<()> {
        // Is this ordering even working when the thread is using Relaxed? But regardless of ordering,
        //  thread spawning should have the atomic been done.
        this.running.store(true, Ordering::SeqCst);

        std::thread::Builder::new()
            .name("Physics Tick Thread".into())
            .spawn(move || {
                Self::run(&this);
            })
            .expect("Spawning Physics Thread")
    }

    pub fn stop(&self) {
        self.running.store(false, Ordering::Relaxed);
    }

    fn run(this: &PhysicsState) {
        const TICK_RATE_MS: f32 = 1000.0 / 60.0;
        let mut time_passed: f32 = TICK_RATE_MS;

        while this.running.load(Ordering::Relaxed) {
            let start = Instant::now();

            if time_passed >= TICK_RATE_MS {
                let delta_movement = {
                    let mut lock = this.delta_movement.lock().expect("delta movement lock");
                    let result = *lock;
                    *lock = Vec3::ZERO;
                    result
                };

                let player_movement_info = this.update_fixed(delta_movement);

                if let Some(network) = this.app().network.as_ref() {
                    network
                        .world_server
                        .movement_tracker
                        .write()
                        .expect("Movement Tracker Write Lock tainted")
                        .track_movement(player_movement_info);
                } // Otherwise: Standalone mode. We need a better API

                time_passed -= TICK_RATE_MS; // tick we did

                if time_passed >= TICK_RATE_MS {
                    warn!("Physics tick underrun by {}ms", time_passed);
                }

                if time_passed >= 1000.0 {
                    warn!("Physics skipping a second (\"running slower\")");
                    time_passed -= 1000.0; // Otherwise we will build up a backlog that we will never be able to catch up.
                }
            } else {
                // technically, we should pin the CPU, but we try to reduce the load a bit.
                std::thread::sleep(std::time::Duration::from_millis(10));
            }

            time_passed += start.elapsed().as_millis_f32(); // time we took to calculate or sleep
        }
    }
}
