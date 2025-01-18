use glam::Vec3;
use wow_world_messages::wrath::{MovementBlock_MovementFlags_SplineEnabled, MovementBlock_SplineFlag, Vector3d};

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
