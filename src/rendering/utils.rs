use crate::rendering::common::coordinate_systems::TILE_SIZE;
use glam::{Affine3A, EulerRot, Quat, Vec3};
use sargerust_files::adt::types::SMDoodadDef;
use sargerust_files::wdt::types::SMMapObjDef;

// TODO: this is probably even too specific for here and belongs somewhere in loaders/graph modules.
pub fn transform_for_doodad_ref(dad_ref: &SMDoodadDef) -> Affine3A {
    let scale = Vec3::new(
        dad_ref.scale as f32 / 1024.0,
        dad_ref.scale as f32 / 1024.0,
        dad_ref.scale as f32 / 1024.0,
    );
    let rotation = Quat::from_euler(
        EulerRot::ZYX,
        (dad_ref.rotation.y + 90.0).to_radians(),
        (dad_ref.rotation.x + 0.0).to_radians(),
        (dad_ref.rotation.z + 0.0).to_radians(),
    );
    // MDDFS (TODO: MODF) uses a completely different coordinate system, so we need to fix up things.

    // 32*TILE_SIZE because the map is 64 TS wide, and so we're placing ourselfs into the mid.
    let translation = Vec3::new(
        32.0 * TILE_SIZE - dad_ref.position.x,
        -(32.0 * TILE_SIZE - dad_ref.position.z),
        dad_ref.position.y,
    );
    Affine3A::from_scale_rotation_translation(scale, rotation, translation)
}

pub fn transform_for_wmo_ref(wmo_ref: &SMMapObjDef) -> Affine3A {
    // cfg[feature = "legion")] // Apparently, this scale is only valid starting legion, before it is padding (and probably 0)
    // let scale = Vec3::new(wmo_ref.scale as f32 / 1024.0, wmo_ref.scale as f32 / 1024.0, wmo_ref.scale as f32 / 1024.0);
    let scale = Vec3::new(1.0, 1.0, 1.0);
    let rotation = Quat::from_euler(
        EulerRot::ZYX,
        (wmo_ref.rot.y + 0.5 * 180.0).to_radians(),
        (wmo_ref.rot.x).to_radians(),
        (wmo_ref.rot.z + 0.0).to_radians(),
    );

    // 32*TILE_SIZE because the map is 64 TS wide, and so we're placing ourselfs into the mid.
    let translation = Vec3::new(
        32.0 * TILE_SIZE - wmo_ref.pos.x,
        -(32.0 * TILE_SIZE - wmo_ref.pos.z),
        wmo_ref.pos.y,
    );
    Affine3A::from_scale_rotation_translation(scale, rotation, translation)
}
