use std::collections::HashMap;
use std::fs::File;
use std::io::{BufWriter, Write};
use std::ops::Deref;
use std::rc::Rc;
use glam::{Affine3A, EulerRot, Quat, Vec3};
use image_blp::BlpImage;
use image_blp::convert::blp_to_image;
use image_blp::parser::parse_blp_with_externals;
use itertools::Itertools;
use log::warn;
use mpq::Archive;
use rend3::types::Mesh;
use sargerust_files::adt::reader::ADTReader;
use sargerust_files::adt::types::{ADTAsset, MCCVSubChunk, MCNKChunk, SMDoodadDef};
use sargerust_files::common::types::{C3Vector, C4Quaternion, CImVector};
use sargerust_files::m2::reader::M2Reader;
use sargerust_files::m2::types::{M2Asset, M2SkinProfile};
use sargerust_files::wdt::types::SMMapObjDef;
use sargerust_files::wmo::reader::WMOReader;
use sargerust_files::wmo::types::{WMOGroupAsset, WMORootAsset};
use crate::io::common::loader::RawAssetLoader;
use crate::io::mpq::loader::MPQLoader;
use crate::rendering::importer::m2_importer::M2Importer;

mod io;
mod rendering;

// TODO mpq crate: get rid of getopts
// TODO mpq crate: implement Read instead of it's custom read into a Vec and then reading that?

// TODO
fn from_vec(value: C3Vector) -> Vec3 {
    Vec3::new(value.x, value.y, value.z)
}

fn from_quat(value: C4Quaternion) -> Quat { Quat::from_xyzw(value.x, value.y, value.z, value.w) }

const CHUNK_SIZE: f32 = 100.0/3.0; // 33.333 yards (100 feet)
const GRID_SIZE: f32 = CHUNK_SIZE / 8.0;
const TILE_SIZE: f32 = 16.0 * CHUNK_SIZE;

fn main() {
    env_logger::init();

    // TODO: perspectively, this folder will be a CLI argument
    let data_folder = std::env::current_dir().expect("Can't read current working directory!").join("_data");

    let mut mpq_loader = io::mpq::loader::MPQLoader::new(data_folder.to_string_lossy().as_ref());
    //let kalimdor_wdt = mpq_loader.load_raw_owned(r"World\Maps\Kalimdor\Kalimdor.wdt").expect("Could locate kalimdor.wdt");

    // debug_dump_mpq_filelist(data_folder, "common.MPQ");
    // debug_dump_mpq_filelist(data_folder, "common-2.MPQ");
    // debug_dump_mpq_filelist(data_folder, "patch.MPQ");
    // debug_dump_mpq_filelist(data_folder, "patch-2.MPQ");
    // debug_dump_mpq_filelist(data_folder, "patch-3.MPQ");
    // debug_dump_mpq_filelist(data_folder, "expansion.MPQ");
    // debug_dump_mpq_filelist(data_folder, "lichking.MPQ");

    let simple_m2 = true;
    let simple_wmo = false;
    let simple_adt = false;

    // let mut common = Archive::open(data_folder.join("common.MPQ")).unwrap();
    // let mut common2 = Archive::open(data_folder.join("common-2.MPQ")).unwrap();

    // debug_dump_file(&mut common2, r"World\Maps\DeeprunTram\DeeprunTram.wdl");
    // debug_dump_file(&mut common2, r"World\Maps\DeeprunTram\DeeprunTram.wdt");
    //debug_dump_file(&mut common2, "World\\wmo\\Dungeon\\AZ_Subway\\Subway.wmo");

    // GM Island ;)
    // debug_dump_file(&mut common2, r"World\Maps\Kalimdor\Kalimdor_0_0.adt");
    // debug_dump_file(&mut common2, r"World\Maps\Kalimdor\Kalimdor_0_1.adt");
    // debug_dump_file(&mut common2, r"World\Maps\Kalimdor\Kalimdor_0_2.adt");

    if simple_m2 {
        main_simple_m2(&mut mpq_loader).unwrap();
    }
    else if simple_wmo {
        main_simple_wmo(&mut mpq_loader).unwrap();
    } else if simple_adt {
        main_simple_adt(&mut mpq_loader).unwrap();
    } else {
        main_multiple_adt(&mut mpq_loader).unwrap();
    }
}


/// Extracts the doodads (i.e. M2 models that have been placed into the world at a specific position) that are defined in the WMO Root
fn collect_dooads(loader: &mut MPQLoader, m2_cache: &mut HashMap<String, Rc<(rendering::common::types::Mesh, Option<BlpImage>)>>, wmo: &WMORootAsset) -> Vec<(Affine3A, Rc<(rendering::common::types::Mesh, Option<BlpImage>)>)> {
    let mut render_list: Vec<(Affine3A, Rc<(rendering::common::types::Mesh, Option<BlpImage>)>)> = Vec::new();
    for mods in &wmo.mods.doodadSetList {
        let start = mods.startIndex as usize;
        let end = (mods.startIndex + mods.count) as usize;
        println!("Doodad Set: {} from {} to {}", mods.name, start, end);
        for modd in &wmo.modd.doodadDefList[start..end] {
            let idx = wmo.modn.doodadNameListLookup[&modd.nameIndex];
            let name = wmo.modn.doodadNameList[idx].as_str();

            // fix name: currently it ends with .mdx but we need .m2
            let name = name.replace(".MDX", ".m2").replace(".MDL", ".m2");
            if name.to_lowercase().contains("emitter") {
                continue;
            }

            let entry = load_m2_doodad(loader, m2_cache, &name);
            let transform: Affine3A = Affine3A::from_scale_rotation_translation(Vec3::new(modd.scale, modd.scale, modd.scale), from_quat(modd.orientation), from_vec(modd.position));
            render_list.push((transform, entry.clone()));
        }
    }

    render_list
}

fn load_m2_doodad(loader: &mut MPQLoader, m2_cache: &mut HashMap<String, Rc<(rendering::common::types::Mesh, Option<BlpImage>)>>, name: &String) -> Rc<(rendering::common::types::Mesh, Option<BlpImage>)> {
    // TODO: this should be called by the simple_m2 path
    let entry = m2_cache.entry(name.clone()).or_insert_with(|| {
        let mut m2_file = std::io::Cursor::new(loader.load_raw_owned(&name).unwrap());
        let m2_asset = M2Reader::parse_asset(&mut m2_file).unwrap();
        // In theory, we could investigate the number of LoD Levels, but we will just use "0"
        let mut skin_file =  std::io::Cursor::new(loader.load_raw_owned(&name.replace(".m2", "00.skin")).unwrap());
        let skin = M2Reader::parse_skin_profile(&mut skin_file).unwrap();

        let mut blp_opt = None;
        if !m2_asset.textures.is_empty() {
            blp_opt = load_blp_from_ldr(loader, &m2_asset.textures[0].filename);
        }

        let imported_mesh = M2Importer::create_mesh(&m2_asset, &skin);
        dbg!(&imported_mesh);

        Rc::new((imported_mesh, blp_opt))
    });
    entry.clone()
}

fn load_wmo_groups(loader: &mut MPQLoader, wmo: &WMORootAsset, path: &str) -> Vec<WMOGroupAsset> {
    for group in &wmo.mogi.groupInfoList {
        if group.nameoffset != -1
        {
            let offset = wmo.mogn.offset_lookup[&(group.nameoffset as u32)];
            dbg!(&wmo.mogn.groupNameList[offset]);
        }
    }

    let mut group_list = Vec::new();
    for x in 0..wmo.mohd.nGroups {
        let cursor = &mut std::io::Cursor::new(
            loader.load_raw_owned(&format!("{}_{:0>3}.wmo", path, x)).unwrap());
        let group = WMOReader::parse_group(cursor).unwrap();
        group_list.push(group);
    }

    group_list
}

fn main_simple_m2(loader: &mut MPQLoader) -> Result<(), anyhow::Error> {
    // This method demonstrates very simple m2 rendering (not in the context of wmos or adts).

    //debug_dump_blp(&mut expansion, r"WORLD\EXPANSION01\DOODADS\GENERIC\BLOODELF\BENCHES\BE_BENCH_01.BLP");
    //let m2_path = "World\\EXPANSION01\\DOODADS\\GENERIC\\BLOODELF\\Chairs\\BE_Chair01.m2";
    //let skin_path =  r"World\EXPANSION01\DOODADS\GENERIC\BLOODELF\Chairs\BE_Chair0100.skin";
    //let tex_path = r"WORLD\EXPANSION01\DOODADS\GENERIC\BLOODELF\BENCHES\BE_BENCH_01.BLP";

    let m2_path = r"Creature\talbuk\Talbuk.m2";
    let skin_path = r"Creature\talbuk\Talbuk00.skin";
    let tex_path = r"Creature\talbuk\TalbukSkinBrown.blp"; // common.mpq!

    let m2 = M2Reader::parse_asset(&mut std::io::Cursor::new(loader.load_raw_owned(m2_path).unwrap()))?;
    let skin = M2Reader::parse_skin_profile(&mut std::io::Cursor::new(loader.load_raw_owned(skin_path).unwrap()))?;
    let blp_opt = load_blp_from_ldr(loader, tex_path);
    let imported_mesh = M2Importer::create_mesh(&m2, &skin);
    dbg!(&imported_mesh);

    // Note: This API is already a bad monstrosity, it WILL go, but it makes prototyping easier.
    rendering::render(vec![(Affine3A::from_translation(Vec3::new(0.0, 0.0, 0.0)),
                            Rc::new((imported_mesh, blp_opt)))], vec![],
                      HashMap::new(), vec![]);
    Ok(())
}

fn main_simple_wmo(loader: &mut MPQLoader) -> Result<(), anyhow::Error> {
    // This method demonstrates very simple wmo rendering (not in the context of adts).
    //let wmo_path = r"World\wmo\Dungeon\AZ_Subway\Subway";
    //let wmo_path = r"World\wmo\Azeroth\Buildings\GoldshireInn\GoldshireInn"; // good example of how we need to filter doodad sets
    //let wmo_path = r"World\wmo\Azeroth\Buildings\GriffonAviary\GriffonAviary"; // <-- orange color, no textures?
    let wmo_path = r"World\wmo\Azeroth\Buildings\GoldshireInn\GoldshireInn_closed";

    let mut wmo: WMORootAsset = WMOReader::parse_root(&mut std::io::Cursor::new(loader.load_raw_owned(&format!("{}.wmo", wmo_path)).unwrap()))?;

    // TODO: m2_cache should become an implementation detail of the struct that provides collect_doodads?
    // TODO: collect_doodads shouldn't contain the cache loading logic anyway.
    let mut m2_cache: HashMap<String, Rc<( rendering::common::types::Mesh, Option<BlpImage>)>> = HashMap::new();

    let dooads = collect_dooads(loader, &mut m2_cache, &mut wmo);
    let group_list = load_wmo_groups(loader, &wmo, wmo_path);
    let textures = wmo.momt.materialList.iter()
        .map(|tex| wmo.motx.textureNameList[wmo.motx.offsets[&tex.texture_1]].clone())
        .collect_vec();

    let mut texture_map = HashMap::new();
    for texture in textures {
        let blp = load_blp_from_ldr(loader, &texture).expect("Texture loading error");
        texture_map.insert(texture, blp);
    }

    let wmos = vec![(Affine3A::IDENTITY, wmo, group_list)];

    // Note: This API is already a bad monstrosity, it WILL go, but it makes prototyping easier.
    rendering::render(dooads, wmos.iter().map(|wmo| (&wmo.0, &wmo.1, &wmo.2)), texture_map,
                      vec![]);
    Ok(())
}

fn main_simple_adt(loader: &mut MPQLoader) -> Result<(), anyhow::Error> {
    let adt = ADTReader::parse_asset(&mut std::io::Cursor::new(loader.load_raw_owned(r"World\Maps\Kalimdor\Kalimdor_1_1.adt").unwrap()))?;

    let mut m2_cache: HashMap<String, Rc<(rendering::common::types::Mesh, Option<BlpImage>)>> = HashMap::new();
    let mut render_list: Vec<(Affine3A, Rc<(rendering::common::types::Mesh, Option<BlpImage>)>)> = Vec::new();
    let mut texture_map = HashMap::new();
    let mut wmos: Vec<(Affine3A, WMORootAsset, Vec<WMOGroupAsset>)> = Vec::new();

    let terrain_chunk = handle_adt(loader, adt, &mut m2_cache, &mut render_list, &mut texture_map, &mut wmos)?;
    rendering::render(render_list, wmos.iter().map(|wmo| (&wmo.0, &wmo.1, &wmo.2)), texture_map, terrain_chunk);
    Ok(())
}

fn main_multiple_adt(loader: &mut MPQLoader) -> Result<(), anyhow::Error> {
    // technically, wdt loading doesn't differ all too much, because if it has terrain, it doesn't have it's own dooads
    // and then all you have to check is for existing adt files (MAIN chunk)
    let map_name = r"World\Maps\Kalimdor\Kalimdor";
    let mut m2_cache: HashMap<String, Rc<(rendering::common::types::Mesh, Option<BlpImage>)>> = HashMap::new();
    let mut render_list: Vec<(Affine3A, Rc<(rendering::common::types::Mesh, Option<BlpImage>)>)> = Vec::new();
    let mut texture_map = HashMap::new();
    let mut wmos: Vec<(Affine3A, WMORootAsset, Vec<WMOGroupAsset>)> = Vec::new();
    let mut terrain_chunks: Vec<(C3Vector, Vec<(Vec3, CImVector)>, Vec<u32>)> = Vec::new();

    for row in 0..2 {
        for column in 0..2 {
            let adt = ADTReader::parse_asset(&mut std::io::Cursor::new(loader.load_raw_owned(&format!("{}_{}_{}.adt", map_name, row, column)).unwrap()))?;
            terrain_chunks.extend(handle_adt(loader, adt, &mut m2_cache, &mut render_list, &mut texture_map, &mut wmos)?);
        }
    }

    rendering::render(render_list, wmos.iter().map(|wmo| (&wmo.0, &wmo.1, &wmo.2)), texture_map, terrain_chunks);
    Ok(())
}

fn handle_adt(loader: &mut MPQLoader, adt: Box<ADTAsset>, m2_cache: &mut HashMap<String, Rc<(rendering::common::types::Mesh, Option<BlpImage>)>>, render_list: &mut Vec<(Affine3A, Rc<(rendering::common::types::Mesh, Option<BlpImage>)>)>, texture_map: &mut HashMap<String, BlpImage>, wmos: &mut Vec<(Affine3A, WMORootAsset, Vec<WMOGroupAsset>)>) -> Result<Vec<(C3Vector, Vec<(Vec3, CImVector)>, Vec<u32>)>, anyhow::Error> {
    for wmo_ref in adt.modf.mapObjDefs.iter() {
        let name = &adt.mwmo.filenames[*adt.mwmo.offsets.get(&adt.mwid.mwmo_offsets[wmo_ref.nameId as usize]).unwrap()];
        //dbg!(&name);

        let wmo = WMOReader::parse_root(&mut std::io::Cursor::new(loader.load_raw_owned(name).unwrap())).unwrap();
        let transform = transform_for_wmo_ref(wmo_ref);

        let doodads = collect_dooads(loader, m2_cache, &wmo);
        for dad in doodads {
            // NOTE: Here we loose the relationship between DAD and wmo, that is required for parenting.
            // Since rend3 does not have a scenegraph, we "fake" the parenting for now.
            let (mut dad_trans, rc) = dad;
            //dbg!(dad_trans.translation);
            dad_trans = transform * dad_trans;
            //dbg!(dad_trans.translation);
            render_list.push((dad_trans, rc));
        }

        // preload (force-cache) all textures
        let textures = wmo.momt.materialList.iter()
            .map(|tex| wmo.motx.textureNameList[wmo.motx.offsets[&tex.texture_1]].clone())
            .collect_vec();

        for texture in textures {
            let blp = load_blp_from_ldr(loader, &texture).expect("Texture loading error");
            texture_map.insert(texture, blp);
        }

        let group_list = load_wmo_groups(loader, &wmo, name.trim_end_matches(".wmo").trim_end_matches(".WMO"));

        wmos.push((transform, wmo, group_list));
    }

    // TODO: deduplicate with collect doodads (at least the emitter and m2 name replacement)
    for dad_ref in adt.mddf.doodadDefs {
        let name = &adt.mmdx.filenames[*adt.mmdx.offsets.get(&adt.mmid.mmdx_offsets[dad_ref.nameId as usize]).unwrap()];
        dbg!(&name);

        // fix name: currently it ends with .mdx but we need .m2
        let name = name.to_lowercase().replace(".mdx", ".m2").replace(".mdl", ".m2");
        if name.to_lowercase().contains("emitter") {
            continue;
        }

        let entry = load_m2_doodad(loader, m2_cache, &name);
        let transform = transform_for_doodad_ref(dad_ref);
        render_list.push((transform, entry.clone()));
    }

    let mut terrain_chunk = vec![];

    for mcnk in adt.mcnks {
        let mut index_buffer = Vec::<u32>::new();
        let mut vert_list = Vec::new();
        let mcvt = mcnk.get_mcvt()?.unwrap();
        let use_vertex_color: bool = true; // In theory with this flag we can turn it off for debug purposes.
        //let mccv_opt = mcnk.get_mccv()?.filter(|_| use_vertex_color); // smchunk flag has_mccv.
        let mccv_opt: Option<MCCVSubChunk> = None; // TODO: Fixme

        // Here we're in ADT Terrain space, that is +x -> north, +y -> west. Thus rows grow in -x, columns go to -y.
        // index of 9x9: 17 * row + column
        // index of high detail 8x8: 17 * row + column + 9
        for row in 0..9 {
            for column in 0..9 {
                let low = MCNKChunk::get_index_low(row, column);
                let height = mcvt[low as usize];
                // TODO: implement MCCV the _rust_ way (can't unwrap in a loop)
                // let color = if &mccv_opt.is_some() { (mccv_opt.unwrap()[low as usize]) } else { CImVector::from(0x0000FFFFu32) };
                let color = CImVector::from(0x0000FFFFu32);
                vert_list.push((Vec3::new(-GRID_SIZE * row as f32, -GRID_SIZE * column as f32, height), color));
            }

            if row == 8 {
                continue;
            }

            for column in 0..8 {
                let high = MCNKChunk::get_index_high(row, column);
                let height = mcvt[high as usize];
                // see above
                // let color = if use_vertex_color { (&mccv_opt.unwrap()[high as usize]).clone() } else { CImVector::from(0xFF0000FFu32) };
                let color = CImVector::from(0xFF0000FFu32);
                vert_list.push((Vec3::new(-GRID_SIZE * (row as f32 + 0.5), -GRID_SIZE * (column as f32 + 0.5), height), color));
            }
        }

        // build the index buffer, this is probably the most difficult part.
        let low_res = false;
        if low_res {
            for row in 0..8 { // last row won't work.
                for column in 0..8 {
                    // tri 1
                    index_buffer.push(MCNKChunk::get_index_low(row, column) as u32);
                    index_buffer.push(MCNKChunk::get_index_low(row, column + 1) as u32);
                    index_buffer.push(MCNKChunk::get_index_low(row + 1, column) as u32);

                    // tri 2
                    index_buffer.push(MCNKChunk::get_index_low(row, column + 1) as u32);
                    index_buffer.push(MCNKChunk::get_index_low(row + 1, column + 1) as u32);
                    index_buffer.push(MCNKChunk::get_index_low(row + 1, column) as u32);
                }
            }
        } else {
            for row in 0..8 { // last row won't work.
                for column in 0..8 {
                    // W
                    index_buffer.push(MCNKChunk::get_index_low(row, column) as u32);
                    index_buffer.push(MCNKChunk::get_index_high(row, column) as u32);
                    index_buffer.push(MCNKChunk::get_index_low(row + 1, column) as u32);

                    // N
                    index_buffer.push(MCNKChunk::get_index_low(row, column) as u32);
                    index_buffer.push(MCNKChunk::get_index_low(row, column + 1) as u32);
                    index_buffer.push(MCNKChunk::get_index_high(row, column) as u32);

                    // E
                    index_buffer.push(MCNKChunk::get_index_low(row, column + 1) as u32);
                    index_buffer.push(MCNKChunk::get_index_low(row + 1, column + 1) as u32);
                    index_buffer.push(MCNKChunk::get_index_high(row, column) as u32);

                    // S
                    index_buffer.push(MCNKChunk::get_index_low(row + 1, column) as u32);
                    index_buffer.push(MCNKChunk::get_index_high(row, column) as u32);
                    index_buffer.push(MCNKChunk::get_index_low(row + 1, column + 1) as u32);
                }
            }
        }

        // let mut w = BufWriter::new(File::create("./terrain.obj")?);
        // writeln!(w, "o {}","terrain")?;
        // for v in vert_list {
        //     let (vert, col) = v;
        //     writeln!(w, "v {} {} {}", vert.x, vert.y, vert.z)?;
        // }
        //
        // for i in index_buffer.chunks_exact(3) {
        //     writeln!(w, "f {} {} {}", i[0] + 1, i[1] + 1, i[2] + 1)?;
        // }

        let chunk_height = mcnk.header.position.z;
        let x = mcnk.header.position.x;
        let y = mcnk.header.position.y;
        let pos = C3Vector { x, y, z: chunk_height };
        dbg!(pos);

        terrain_chunk.push((pos, vert_list, index_buffer));
    }
    Ok(terrain_chunk)
}

fn transform_for_doodad_ref(dad_ref: SMDoodadDef) -> Affine3A {
    let scale = Vec3::new(dad_ref.scale as f32 / 1024.0, dad_ref.scale as f32 / 1024.0, dad_ref.scale as f32 / 1024.0);
    //let rotation = Quat::from_euler(EulerRot::ZYX, dad_ref.rotation.x.to_radians(), (dad_ref.rotation.y - 90.0).to_radians(), dad_ref.rotation.z.to_radians());
    let rotation = Quat::from_euler(EulerRot::ZYX, (dad_ref.rotation.y + 180.0).to_radians(), (dad_ref.rotation.x + 0.0).to_radians(), (dad_ref.rotation.z + 0.0).to_radians());
    // MDDFS (TODO: MODF) uses a completely different coordinate system, so we need to fix up things.

    // 32*TILE_SIZE because the map is 64 TS wide, and so we're placing ourselfs into the mid.
    let translation = Vec3::new((32.0 * TILE_SIZE - dad_ref.position.x), -(32.0 * TILE_SIZE - dad_ref.position.z), dad_ref.position.y);
    dbg!(translation);

    let transform: Affine3A = Affine3A::from_scale_rotation_translation(scale, rotation, translation);
    transform
}

fn transform_for_wmo_ref(wmo_ref: &SMMapObjDef) -> Affine3A {
    // Apparently, this scale is only valid starting legion, before it is padding (and probably 0)
    // cfg[feature = "legion")]
    // let scale = Vec3::new(wmo_ref.scale as f32 / 1024.0, wmo_ref.scale as f32 / 1024.0, wmo_ref.scale as f32 / 1024.0);

    let scale = Vec3::new(1.0, 1.0, 1.0);
    //let rotation = Quat::from_euler(EulerRot::ZYX, wmo_ref.rot.x.to_radians(), (wmo_ref.rot.y - 90.0).to_radians(), (wmo_ref.rot.z + 0.0).to_radians());
    let rotation = Quat::from_euler(EulerRot::ZYX, (wmo_ref.rot.y + 0.5*180.0).to_radians(), (wmo_ref.rot.x).to_radians(), (wmo_ref.rot.z + 0.0).to_radians());
    // let mut translation = from_vec(wmo_ref.pos);
    // // MODF uses a completely different coordinate system, so we need to fix up things.
    // translation.x = -translation.x; // west is positive X!!
    // std::mem::swap(&mut translation.z, &mut translation.y); // maybe needs inverting.

    // // relative to a corner of the map, but we want to have the center in mid (just alone for rotation etc)
    // translation.x -= 17066.0; // WOWDEV
    // translation.y -= 17066.0;

    // 32*TILE_SIZE because the map is 64 TS wide, and so we're placing ourselfs into the mid.
    let translation = Vec3::new((32.0 * TILE_SIZE - wmo_ref.pos.x), -(32.0 * TILE_SIZE - wmo_ref.pos.z), wmo_ref.pos.y);
    dbg!(translation);
    let transform: Affine3A = Affine3A::from_scale_rotation_translation(scale, rotation, translation);
    transform
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

fn load_blp_from_ldr(mpq_loader: &mut MPQLoader, file_name: &str) -> Option<BlpImage> {
    // TODO: The blp crate has bad error handling, as it doesn't mix with anyhow::Error.
    // furthermore, the built in error types stem from nom, that we don't have as dependency.

    // load_blp uses the fs to load mip maps next to it.
    // we don't want to extract blps into temporary files, though, so we use the other API
    // and there, we either don't support BLP0 Mipmaps or we properly implement the callback at some time

    let owned_file = mpq_loader.load_raw_owned(file_name);
    if owned_file.is_none() {
        warn!("Could not load BLP {}", file_name);
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
