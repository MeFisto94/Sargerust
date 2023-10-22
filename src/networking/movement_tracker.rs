use crate::networking::world::WorldServer;
use crate::physics::character_movement_information::CharacterMovementInformation;
use glam::{Quat, Vec3};
use std::f32::consts::PI;
use std::sync::Weak;
use std::time::Instant;
use wow_world_messages::wrath::{
    MovementFlags, MovementInfo, MovementInfo_MovementFlags, Vector3d, MSG_MOVE_HEARTBEAT, MSG_MOVE_START_BACKWARD,
    MSG_MOVE_START_FORWARD, MSG_MOVE_START_STRAFE_LEFT, MSG_MOVE_START_STRAFE_RIGHT, MSG_MOVE_START_TURN_LEFT,
    MSG_MOVE_START_TURN_RIGHT, MSG_MOVE_STOP,
};

/// The Movement Tracker is the struct responsible for sending the CMSG MOVE packets for the current player.
/// It has nothing to do with tracking movement of other entities!
pub struct MovementTracker {
    world_server: Weak<WorldServer>,
    last_movement_info: MovementInfo,
    last_orientation: f32,
    last_heartbeat: Instant,
}

impl MovementTracker {
    pub fn new(world_server: Weak<WorldServer>) -> Self {
        Self {
            world_server,
            last_movement_info: MovementInfo::default(),
            last_orientation: 0.0,
            last_heartbeat: Instant::now(),
        }
    }

    pub fn track_movement(&mut self, movement_info: CharacterMovementInformation) {
        let counter_rotation = Quat::from_rotation_z(-movement_info.orientation);
        let delta_unrotated = counter_rotation * movement_info.delta_movement;

        self._track_movement(
            delta_unrotated,
            movement_info.absolute_position,
            movement_info.orientation,
        );
    }

    fn _track_movement(&mut self, delta_unrotated: Vec3, absolute_position: Vec3, orientation: f32) {
        let world = self
            .world_server
            .upgrade()
            .expect("World Server to outlive Movement Tracker");

        let player_guid = world.player_guid.get().expect("Player Guid is already set");
        let timestamp = world.get_timestamp();

        let info = Self::build_movement_info(
            delta_unrotated,
            absolute_position,
            // TODO: this orientation swap only works for the start/stop command, not for the heartbeat apparently? tapping "W" has the right facing, continuous walking doesn't
            //  this may be related to the heartbeat and/or us not emitting START_TURN_XX messages as of now.
            // TODO: apparently this correct factor is dependant on the initial facing? So we need a different solution!
            orientation - 0.75 * PI, /* TODO: do we need the same correction factor for other units? */
            timestamp,
        );
        let info_clone = info.clone();

        // TODO: integrate into the following if-else branch. It has been commented out for the time being.
        // if orientation != self.last_orientation {
        //     if orientation < self.last_orientation {
        //         world
        //             .send_encrypted(MSG_MOVE_START_TURN_LEFT {
        //                 guid: *player_guid,
        //                 info: info.clone(),
        //             })
        //             .expect("Sending message to be successful");
        //     } else {
        //         world
        //             .send_encrypted(MSG_MOVE_START_TURN_RIGHT {
        //                 guid: *player_guid,
        //                 info: info.clone(),
        //             })
        //             .expect("Sending message to be successful");
        //     }
        //
        //     self.last_orientation = orientation;
        // }

        if info.flags.is_empty() {
            if Self::is_moving(&self.last_movement_info.flags) {
                // We've been moving, so stop.
                let msg = MSG_MOVE_STOP {
                    guid: *player_guid,
                    info,
                };
                world
                    .send_encrypted(msg)
                    .expect("Sending message to be successful");
                self.last_heartbeat = Instant::now();
            } // else: do nothing, we're standing still.
        } else if self.last_movement_info.flags != info.flags {
            self.last_heartbeat = Instant::now();
            if info.flags.get_forward() {
                world
                    .send_encrypted(MSG_MOVE_START_FORWARD {
                        guid: *player_guid,
                        info,
                    })
                    .expect("Sending message to be successful");
            } else if info.flags.get_backward() {
                world
                    .send_encrypted(MSG_MOVE_START_BACKWARD {
                        guid: *player_guid,
                        info,
                    })
                    .expect("Sending message to be successful");
            } else if info.flags.get_strafe_left() {
                world
                    .send_encrypted(MSG_MOVE_START_STRAFE_LEFT {
                        guid: *player_guid,
                        info,
                    })
                    .expect("Sending message to be successful");
            } else if info.flags.get_strafe_right() {
                world
                    .send_encrypted(MSG_MOVE_START_STRAFE_RIGHT {
                        guid: *player_guid,
                        info,
                    })
                    .expect("Sending message to be successful");
            }
        } else {
            // TODO: this currently fires a ByteBufferExcepton sometimes when parsing apparently.
            if self.last_heartbeat.elapsed().as_millis() >= 500 {
                world
                    .send_encrypted(MSG_MOVE_HEARTBEAT {
                        guid: *player_guid,
                        info,
                    })
                    .expect("Sending message to be successful");
                self.last_heartbeat = Instant::now();
            }
        }

        self.last_movement_info = info_clone;
    }

    fn build_movement_info(
        delta_unrotated: Vec3,
        absolute_position: Vec3,
        orientation: f32,
        timestamp: u32,
    ) -> MovementInfo {
        let inner_flags = Self::build_movement_flags(delta_unrotated);

        MovementInfo {
            flags: MovementInfo_MovementFlags::new(
                inner_flags.as_int(),
                None, // TODO:
                None,
                None,
                None,
            ),
            timestamp,
            position: Vector3d {
                x: absolute_position.x,
                y: absolute_position.y,
                z: absolute_position.z,
            },
            orientation,
            fall_time: 0.0, // TODO
        }
    }

    /// Converts the actual movement input into MovementFlags.
    /// delta_unrotated means it's in the characters local space (blender space?)
    fn build_movement_flags(delta_unrotated: Vec3) -> MovementFlags {
        const EPSILON: f32 = 1.0e-3; // relatively large to prevent physics drift from causing a movement
        if delta_unrotated.abs_diff_eq(Vec3::ZERO, f32::EPSILON) {
            return MovementFlags::new(MovementFlags::NONE);
        }

        let mut flags = MovementFlags::new(0);

        if delta_unrotated.z.is_sign_negative() && delta_unrotated.z.abs() > EPSILON {
            flags.set_falling();
            // TODO: this implies setting fall_time at the very least but also a few more flags on MovementInfo
            return flags; // No chance to walk or do anything else.
        }

        if delta_unrotated.x.abs() > EPSILON {
            if delta_unrotated.x.is_sign_negative() {
                flags.set_strafe_left();
            } else {
                flags.set_strafe_right();
            }
        }

        if delta_unrotated.z.abs() > EPSILON {
            if delta_unrotated.z.is_sign_negative() {
                flags.set_backward();
            } else {
                flags.set_forward();
            }
        }

        flags
    }

    const fn is_moving(flags: &MovementInfo_MovementFlags) -> bool {
        flags.get_forward() || flags.get_backward() || flags.get_strafe_left() || flags.get_strafe_right()
    }
}
