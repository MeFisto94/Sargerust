use glam::{Vec3, Vec4};

pub mod weak_dashmap;

pub fn int_as_color(data: i32) -> Vec4 {
    Vec4::new(
        ((data >> 16) & 0xFF) as f32 / 255.0,
        ((data >> 8) & 0xFF) as f32 / 255.0,
        (data & 0xFF) as f32 / 255.0,
        1.0,
    )
}

pub fn spherical_to_cartesian(rho: f32, phi: f32, theta: f32) -> Vec3 {
    let sin_phi = phi.sin();
    Vec3::new(sin_phi * theta.cos(), sin_phi * theta.sin(), phi.cos()) * rho
}
