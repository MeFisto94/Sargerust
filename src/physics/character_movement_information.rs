use glam::Vec3;

pub struct CharacterMovementInformation {
    pub delta_movement: Vec3,
    pub absolute_position: Vec3,
    pub orientation: f32,
}
