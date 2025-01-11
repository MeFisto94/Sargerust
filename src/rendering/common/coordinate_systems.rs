use glam::{EulerRot, Mat4, Vec3, Vec3A};
use std::f32::consts::PI;
use std::ops::Add;

/// ADT is RH, Up: Z, East: -Y, North: +X
#[inline]
pub fn adt_to_blender(source: Vec3A) -> Vec3A {
    Vec3A::new(source.y, -source.x, source.z)
    // assert_eq!(res, Mat4::from_euler(EulerRot::XYZ, 0.0, 0.0 * PI, -0.5 * PI).transform_point3a(source)); -> epsilon missing
}

#[inline]
pub fn blender_to_adt(source: Vec3A) -> Vec3A {
    Vec3A::new(-source.y, source.x, source.z)
}

#[inline]
pub fn adt_to_blender_rot() -> Mat4 {
    // flip 90 degrees negative around the Z axis
    Mat4::from_euler(EulerRot::XYZ, 0.0 * PI, 0.0 * PI, -0.5 * PI)
}

#[inline]
pub fn blender_to_adt_rot() -> Mat4 {
    // flip 90 degrees positive around the Z axis
    Mat4::from_euler(EulerRot::XYZ, 0.0 * PI, 0.0 * PI, 0.5 * PI)
}

#[inline]
pub fn adt_to_blender_transform(source: Vec3A) -> Mat4 {
    Mat4::from_translation(adt_to_blender(source).into()) * adt_to_blender_rot()
}

#[inline]
pub fn adt_tiles_to_world(row: u8, column: u8) -> Vec3A {
    // tile (0, 0) is (17066, 17066)
    // tile (32, 32) is (0, 0)
    // tile (64, 64) is (-17066, -17066)
    Vec3A::new(
        (row as f32 - 32.0) * -TILE_SIZE,
        (column as f32 - 32.0) * -TILE_SIZE,
        0.0,
    )
}

#[inline]
pub fn adt_world_to_tiles(position: Vec3) -> (u8, u8) {
    let chunk_coords = (position / -TILE_SIZE)
        .floor()
        .add(Vec3::new(32.0, 32.0, 0.0));
    (chunk_coords.x as u8, chunk_coords.y as u8)
}

const CHUNK_SIZE: f32 = 100.0 / 3.0;
// 33.333 yards (100 feet)
pub const GRID_SIZE: f32 = CHUNK_SIZE / 8.0;
pub const TILE_SIZE: f32 = 16.0 * CHUNK_SIZE;
