use glam::Vec3;
use wow_world_messages::wrath::Vector3d;

pub fn net_vector3d_to_glam(value: Vector3d) -> Vec3 {
    Vec3::new(value.x, value.y, value.z)
}
