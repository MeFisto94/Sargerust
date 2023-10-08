use std::collections::HashMap;
use std::ops::Deref;
use std::sync::Arc;

use glam::{Affine3A, EulerRot, Quat, Vec3};
use image_blp::BlpImage;
use image_blp::convert::blp_to_image;
use image_blp::parser::parse_blp_with_externals;
use itertools::Itertools;
use log::trace;

use mpq::Archive;
use rendering::common::coordinate_systems::TILE_SIZE;
use sargerust_files::adt::types::{ADTAsset, SMDoodadDef};
use sargerust_files::wdt::types::SMMapObjDef;

use crate::game::application::GameApplication;
use crate::io::mpq::loader::MPQLoader;
use crate::rendering::common::highlevel_types::PlacedDoodad;
use crate::rendering::common::types::{AlbedoType, Material, Mesh, MeshWithLod};
use crate::rendering::importer::adt_importer::ADTImporter;
use crate::rendering::loader::blp_loader::BLPLoader;
use crate::rendering::loader::m2_loader::{LoadedM2, M2Loader};
use crate::rendering::loader::wmo_loader::WMOLoader;

mod io;
mod rendering;
mod game;
pub mod networking;
mod demos; // Containing the rendering/application for the Asset Viewers.

enum DemoMode {
    M2,
    WMO,
    ADT,
    MultipleAdt,
    NoDemo
}

fn main() {
    let mode = DemoMode::NoDemo;
    env_logger::init();

    // TODO: perspectively, this folder will be a CLI argument
    let data_folder = std::env::current_dir().expect("Can't read current working directory!").join("_data");
    let mpq_loader = MPQLoader::new(data_folder.to_string_lossy().as_ref());

    match mode {
        DemoMode::M2 => demos::main_simple_m2(&mpq_loader).unwrap(),
        DemoMode::WMO =>  demos::main_simple_wmo(&mpq_loader).unwrap(),
        DemoMode::ADT => demos::main_simple_adt(&mpq_loader).unwrap(),
        DemoMode::MultipleAdt => demos::main_multiple_adt(&mpq_loader).unwrap(),
        DemoMode::NoDemo => {
            let mut recv = None;
            let app = Arc::new_cyclic(|weak| {
                let mut app = GameApplication::new(weak, mpq_loader);
                recv = Some(app.realm_logon("127.0.0.1:3724"));
                app
            });

            app.run(recv.unwrap());
        }
    }
}

fn handle_adt(loader: &MPQLoader, adt: &ADTAsset, m2_cache: &mut HashMap<String, Arc<LoadedM2>>, render_list: &mut Vec<PlacedDoodad>, texture_map: &mut HashMap<String, BlpImage>, wmos: &mut Vec<(Affine3A, Vec<(MeshWithLod, Vec<Material>)>)>) -> Result<Vec<(Vec3, Mesh)>, anyhow::Error> {
    for wmo_ref in adt.modf.mapObjDefs.iter() {
        let name = &adt.mwmo.filenames[*adt.mwmo.offsets.get(&adt.mwid.mwmo_offsets[wmo_ref.nameId as usize]).unwrap()];
        trace!("WMO {} has been referenced from ADT", name);

        if name.ends_with("STORMWIND.WMO") {
            continue; // TODO: Temporary performance optimization
        }

        let loaded = WMOLoader::load(loader, name)?;
        // TODO: currently, only WMO makes use of texture names, M2s load their textures in load_m2_doodad (when the doodad becomes placeable).
        let textures = loaded.loaded_groups.iter()
            .flat_map(|(_, mats)|mats)
            .filter_map(|mat| {
                match &mat.albedo {
                    AlbedoType::TextureWithName(tex_name) => Some(tex_name.clone()),
                    _ => None
                }
            }).collect_vec();

        for texture in textures {
            let blp = BLPLoader::load_blp_from_ldr(loader, &texture).expect("Texture loading error");
            texture_map.insert(texture, blp);
        }

        let transform = transform_for_wmo_ref(wmo_ref);
        for dad in loaded.doodads {
            // NOTE: Here we loose the relationship between DAD and wmo, that is required for parenting.
            // Since rend3 does not have a scenegraph, we "fake" the parenting for now.
            // Also we need to resolve m2 references.
            render_list.push(PlacedDoodad {
                transform: transform * dad.transform,
                m2: load_m2_doodad(loader, m2_cache, &dad.m2_ref),
            });
        }

        wmos.push((transform, loaded.loaded_groups));
    }

    // TODO: deduplicate with collect doodads (at least the emitter and m2 name replacement)
    for dad_ref in &adt.mddf.doodadDefs {
        let name = &adt.mmdx.filenames[*adt.mmdx.offsets.get(&adt.mmid.mmdx_offsets[dad_ref.nameId as usize]).unwrap()];
        trace!("M2 {} has been referenced from ADT", name);

        // fix name: currently it ends with .mdx but we need .m2
        let name = name.to_lowercase().replace(".mdx", ".m2").replace(".mdl", ".m2");
        if name.to_lowercase().contains("emitter") {
            continue;
        }

        let entry = load_m2_doodad(loader, m2_cache, &name);
        render_list.push(PlacedDoodad { transform: transform_for_doodad_ref(dad_ref), m2: entry });
    }

    let mut terrain_chunk = vec![];
    for mcnk in &adt.mcnks {
        terrain_chunk.push(ADTImporter::create_mesh(mcnk)?);
    }

    Ok(terrain_chunk)
}

fn load_m2_doodad(loader: &MPQLoader, m2_cache: &mut HashMap<String, Arc<LoadedM2>>, name: &str) -> Arc<LoadedM2> {
    // Caching M2s is unavoidable in some way, especially when loading multiple chunks in parallel.
    // Otherwise, m2s could be loaded multiple times, but the important thing is to deduplicate
    // them before sending them to the render thread. Share meshes and textures!
    let entry = m2_cache.entry(name.to_string()).or_insert_with(|| {
        Arc::new(M2Loader::load_no_lod(loader, name))
    });
    entry.clone()
}

fn transform_for_doodad_ref(dad_ref: &SMDoodadDef) -> Affine3A {
    let scale = Vec3::new(dad_ref.scale as f32 / 1024.0, dad_ref.scale as f32 / 1024.0, dad_ref.scale as f32 / 1024.0);
    let rotation = Quat::from_euler(EulerRot::ZYX, (dad_ref.rotation.y + 90.0).to_radians(), (dad_ref.rotation.x + 0.0).to_radians(), (dad_ref.rotation.z + 0.0).to_radians());
    // MDDFS (TODO: MODF) uses a completely different coordinate system, so we need to fix up things.

    // 32*TILE_SIZE because the map is 64 TS wide, and so we're placing ourselfs into the mid.
    let translation = Vec3::new(32.0 * TILE_SIZE - dad_ref.position.x, -(32.0 * TILE_SIZE - dad_ref.position.z), dad_ref.position.y);
    Affine3A::from_scale_rotation_translation(scale, rotation, translation)
}

fn transform_for_wmo_ref(wmo_ref: &SMMapObjDef) -> Affine3A {
    // cfg[feature = "legion")] // Apparently, this scale is only valid starting legion, before it is padding (and probably 0)
    // let scale = Vec3::new(wmo_ref.scale as f32 / 1024.0, wmo_ref.scale as f32 / 1024.0, wmo_ref.scale as f32 / 1024.0);
    let scale = Vec3::new(1.0, 1.0, 1.0);
    let rotation = Quat::from_euler(EulerRot::ZYX, (wmo_ref.rot.y + 0.5*180.0).to_radians(), (wmo_ref.rot.x).to_radians(), (wmo_ref.rot.z + 0.0).to_radians());

    // 32*TILE_SIZE because the map is 64 TS wide, and so we're placing ourselfs into the mid.
    let translation = Vec3::new(32.0 * TILE_SIZE - wmo_ref.pos.x, -(32.0 * TILE_SIZE - wmo_ref.pos.z), wmo_ref.pos.y);
    Affine3A::from_scale_rotation_translation(scale, rotation, translation)
}

#[allow(unused)]
fn debug_dump_file(archive: &mut Archive, file: &str) {
    let buf = io::mpq::loader::read_mpq_file_into_owned(archive, file).unwrap();
    std::fs::write(format!("./{}", file.replace('\\', "_")), buf).unwrap();
}

#[allow(unused)]
fn debug_dump_blp(archive: &mut Archive, file_name: &str) {
    let blp = load_blp_from_mpq(archive, file_name).unwrap();
    let image = blp_to_image(&blp, 0).expect("decode");
    image.save(format!("{}.png", file_name.replace("\\", "_"))).expect("saved");
}

#[allow(unused)]
fn debug_dump_mpq_filelist(data_dir: &str, mpq_name: &str) {
    let mut archive = Archive::open(format!("{}\\{}", data_dir, mpq_name)).unwrap();
    let buf = io::mpq::loader::read_mpq_file_into_owned(&mut archive, "(listfile)").unwrap();
    std::fs::write(format!("./{}.txt", mpq_name), buf).unwrap();
}

fn load_blp_from_mpq(archive: &mut Archive, file_name: &str) -> Option<BlpImage> {
    // TODO: The blp crate has bad error handling, as it doesn't mix with anyhow::Error.
    // furthermore, the built in error types stem from nom, that we don't have as dependency.

    // load_blp uses the fs to load mip maps next to it.
    // we don't want to extract blps into temporary files, though, so we use the other API
    // and there, we either don't support BLP0 Mipmaps or we properly implement the callback at some time

    let owned_file = io::mpq::loader::read_mpq_file_into_owned(archive, file_name);
    if owned_file.is_err() {
        dbg!(owned_file.unwrap_err());
        return None;
    }

    let root_input = owned_file.unwrap();
    let image = parse_blp_with_externals(&root_input, |_i| {
        // This could also be no_mipmaps from the image-blp parser crate.
        panic!("Loading of BLP Mip Maps is unsupported. File {}", file_name)
    });

    if image.is_err() {
        dbg!(image.unwrap_err());
        return None
    }
    Some(image.unwrap().1)
}
