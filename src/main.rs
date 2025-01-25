#![feature(iter_array_chunks)]

use crate::game::application::GameApplication;
use crate::io::mpq::loader::MPQLoader;
use crate::settings::{CliArgs, OperationMode};
use clap::Parser;
use glam::{Affine3A, EulerRot, Quat, Vec3};
use image_blp::BlpImage;
use image_blp::convert::blp_to_image;
use image_blp::parser::parse_blp_with_externals;
use mpq::Archive;
use rendering::common::coordinate_systems::TILE_SIZE;
use sargerust_files::adt::types::SMDoodadDef;
use sargerust_files::wdt::types::SMMapObjDef;
use std::sync::Arc;
use wow_world_messages::wrath::Vector3d;

pub mod entity;
mod game;
mod io;
pub mod networking;
pub mod physics;
mod rendering;
mod settings;

fn main() {
    env_logger::init();

    let args = CliArgs::parse();
    log::trace!("Starting with args: {:?}", args);

    let mpq_loader = MPQLoader::new(args.data_dir.as_ref());

    let mut receiver = None;
    let app = Arc::new_cyclic(|weak| {
        let mut app = GameApplication::new(weak, mpq_loader, &args);
        if let OperationMode::Remote {
            server_host,
            server_port,
            username,
            password,
        } = &args.operation_mode
        {
            let address = format!("{}:{}", server_host, server_port);
            receiver = Some(app.connect_to_realm(&address, username, password));
        }
        app
    });

    if let OperationMode::Standalone {
        map_name,
        coordinates,
    } = &args.operation_mode
    {
        app.game_state.change_map_from_string(
            map_name,
            Vector3d {
                x: coordinates.x,
                y: coordinates.y,
                z: coordinates.z,
            },
        );
    }

    let operation_mode = if matches!(args.operation_mode, OperationMode::Standalone { .. }) {
        game::application::GameOperationMode::Standalone
    } else {
        game::application::GameOperationMode::Networked(receiver.unwrap())
    };

    app.run(operation_mode);
}

fn transform_for_doodad_ref(dad_ref: &SMDoodadDef) -> Affine3A {
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

fn transform_for_wmo_ref(wmo_ref: &SMMapObjDef) -> Affine3A {
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

#[allow(unused)]
fn debug_dump_file(archive: &mut Archive, file: &str) {
    let buf = io::mpq::loader::read_mpq_file_into_owned(archive, file).unwrap();
    std::fs::write(format!("./{}", file.replace('\\', "_")), buf).unwrap();
}

#[allow(unused)]
fn debug_dump_blp(archive: &mut Archive, file_name: &str) {
    let blp = load_blp_from_mpq(archive, file_name).unwrap();
    let image = blp_to_image(&blp, 0).expect("decode");
    image
        .save(format!("{}.png", file_name.replace("\\", "_")))
        .expect("saved");
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
        return None;
    }
    Some(image.unwrap().1)
}
