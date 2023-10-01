use std::f32::consts::PI;
use glam::{EulerRot, Mat4, Vec3A};

/// ADT is RH, Up: Z, East: -Y, North: +X
#[inline]
pub fn adt_to_blender(source: Vec3A) -> Vec3A {
    Vec3A::new(source.y, -source.x, source.z)
    // assert_eq!(res, Mat4::from_euler(EulerRot::XYZ, 0.0, 0.0 * PI, -0.5 * PI).transform_point3a(source)); -> epsilon missing
}

#[inline]
pub fn adt_to_blender_rot() -> Mat4 {
    // flip 90 degrees negative around the Z axis
    Mat4::from_euler(EulerRot::XYZ, 0.0 * PI, 0.0 * PI, -0.5 * PI)
}

#[inline]
pub fn adt_to_blender_transform(source: Vec3A) -> Mat4 {
    Mat4::from_translation(adt_to_blender(source).into()) * adt_to_blender_rot()
}