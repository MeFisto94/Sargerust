use glam::Vec3;
use wow_world_messages::wrath::{
    MovementBlock_MovementFlags_SplineEnabled, MovementBlock_SplineFlag, SMSG_MONSTER_MOVE, Vector3d,
};

pub struct TmpLocation(pub Vec3);
pub struct TmpOrientation(pub f32);

pub struct SplineWalker {
    pub nodes: Vec<Vector3d>,
    // TODO: This could be in ticks, which seems to be a tickrate of 100 (10ms), but at the moment, we are ticking the
    //  logic at FPS, so we have an input that is not evenly divisible into u32/tick counts.
    pub duration: f32,
    pub time_passed: f32,
    pub flags: MovementBlock_SplineFlag,
}

// TODO: Clarify
const TICK_RATE: f32 = 100.0;

impl From<&MovementBlock_MovementFlags_SplineEnabled> for SplineWalker {
    fn from(value: &MovementBlock_MovementFlags_SplineEnabled) -> Self {
        Self {
            nodes: value.nodes.clone(),
            duration: value.duration as f32 / TICK_RATE,
            time_passed: value.time_passed as f32 / TICK_RATE,
            flags: value.spline_flags,
        }
    }
}

impl From<&SMSG_MONSTER_MOVE> for SplineWalker {
    fn from(value: &SMSG_MONSTER_MOVE) -> Self {
        Self {
            nodes: value.splines.clone(),
            duration: value.duration as f32 / TICK_RATE,
            time_passed: 0.0,
            flags: MovementBlock_SplineFlag::default(),
            // self.flags = msg.spline_flags; // TODO
            // In theory, we need to calculate time_passed, so that we're at msg.spline_point.
        }
    }
}

impl SplineWalker {
    pub fn update_from(&mut self, msg: &SMSG_MONSTER_MOVE) {
        let new = SplineWalker::from(msg);
        let old_flags = self.flags; // until they are supported...
        *self = new;
        self.flags = old_flags;
    }
}
