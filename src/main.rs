use std::collections::HashMap;
use std::io::Cursor;
use std::ops::Deref;
use std::rc::Rc;
use glam::{Affine3A, EulerRot, Quat, Vec3};
use image_blp::BlpImage;
use image_blp::convert::blp_to_image;
use image_blp::parser::parse_blp_with_externals;
use itertools::Itertools;
use mpq::Archive;
use sargerust_files::adt::reader::ADTReader;
use sargerust_files::common::types::{C3Vector, C4Quaternion};
use sargerust_files::m2::reader::M2Reader;
use sargerust_files::m2::types::{M2Asset, M2SkinProfile};
use sargerust_files::wmo::reader::WMOReader;
use sargerust_files::wmo::types::{WMOGroupAsset, WMORootAsset};

mod io;
mod rendering;

// TODO mpq crate: get rid of getopts
// TODO mpq crate: implement Read instead of it's custom read into a Vec and then reading that?

// TODO
fn from_vec(value: C3Vector) -> Vec3 {
    Vec3::new(value.x, value.y, value.z)
}

fn from_quat(value: C4Quaternion) -> Quat { Quat::from_xyzw(value.x, value.y, value.z, value.w) }

fn main() {
    env_logger::init();

    // TODO: perspectively, this folder will be a CLI argument
    let data_folder = std::env::current_dir().expect("Can't read current working directory!").join("_data");

    // debug_dump_mpq_filelist(data_folder, "common.MPQ");
    // debug_dump_mpq_filelist(data_folder, "common-2.MPQ");
    // debug_dump_mpq_filelist(data_folder, "patch.MPQ");
    // debug_dump_mpq_filelist(data_folder, "patch-2.MPQ");
    // debug_dump_mpq_filelist(data_folder, "patch-3.MPQ");
    // debug_dump_mpq_filelist(data_folder, "expansion.MPQ");
    // debug_dump_mpq_filelist(data_folder, "lichking.MPQ");

    let simple_m2 = false;
    let simple_wmo = false;

    let mut common = Archive::open(data_folder.join("common.MPQ")).unwrap();
    let mut common2 = Archive::open(data_folder.join("common-2.MPQ")).unwrap();

    // debug_dump_file(&mut common2, r"World\Maps\DeeprunTram\DeeprunTram.wdl");
    // debug_dump_file(&mut common2, r"World\Maps\DeeprunTram\DeeprunTram.wdt");
    //debug_dump_file(&mut common2, "World\\wmo\\Dungeon\\AZ_Subway\\Subway.wmo");

    // GM Island ;)
    // debug_dump_file(&mut common2, r"World\Maps\Kalimdor\Kalimdor_0_0.adt");
    // debug_dump_file(&mut common2, r"World\Maps\Kalimdor\Kalimdor_0_1.adt");
    // debug_dump_file(&mut common2, r"World\Maps\Kalimdor\Kalimdor_0_2.adt");

    if simple_m2 {
        let mut expansion = Archive::open(data_folder.join("expansion.MPQ")).unwrap();
        main_simple_m2(&mut common, &mut expansion).unwrap();
    }
    else if simple_wmo {
        main_simple_wmo(&mut common, &mut common2).unwrap();
    } else {
        main_simple_adt(&mut common, &mut common2).unwrap();
    }
}

fn main_simple_adt(common: &mut Archive, common2: &mut Archive) -> Result<(), anyhow::Error> {
    let adt = ADTReader::parse_asset(&mut io::mpq::loader::read_mpq_file_into_cursor(common2, r"World\Maps\Kalimdor\Kalimdor_1_1.adt")?)?;

    let mut m2_cache: HashMap<String, Rc<(M2Asset, Vec<M2SkinProfile>, Option<BlpImage>)>> = HashMap::new();
    let mut render_list: Vec<(Affine3A, Rc<(M2Asset, Vec<M2SkinProfile>, Option<BlpImage>)>)> = Vec::new();
    let mut texture_map = HashMap::new();

    dbg!(&adt.mwmo);
    dbg!(&adt.mwid);
    dbg!(&adt.modf);

    // dbg!(adt.mmdx);
    // dbg!(adt.mmid);
    // dbg!(adt.mddf);

    // TODO: placement information missing.
    let mut wmos: Vec<(WMORootAsset, Vec<WMOGroupAsset>)> = Vec::new();
    for wmo_ref in adt.modf.mapObjDefs.iter().skip(1) { // TODO: Remove skip once we use the orientation
        let name = &adt.mwmo.filenames[*adt.mwmo.offsets.get(&adt.mwid.mwmo_offsets[wmo_ref.nameId as usize]).unwrap()];
        dbg!(&name);

        let wmo = WMOReader::parse_root(&mut io::mpq::loader::read_mpq_file_into_cursor(common2, name).unwrap()).unwrap();

        let doodads = collect_dooads(common, common2, &mut m2_cache, &wmo);
        for dad in doodads {
            render_list.push(dad);
        }

        // preload (force-cache) all textures
        let textures = wmo.momt.materialList.iter()
            .map(|tex| wmo.motx.textureNameList[wmo.motx.offsets[&tex.texture_1]].clone())
            .collect_vec();

        for texture in textures {
            let blp = load_blp_from_mpq(common, &texture).expect("Texture loading error");
            texture_map.insert(texture, blp);
        }

        let group_list = load_wmo_groups(common2, &wmo, name.trim_end_matches(".wmo"));
        wmos.push((wmo, group_list));
    }

    for dad_ref in adt.mddf.doodadDefs {
        let name = &adt.mmdx.filenames[*adt.mmdx.offsets.get(&adt.mmid.mmdx_offsets[dad_ref.nameId as usize]).unwrap()];
        dbg!(&name);

        // fix name: currently it ends with .mdx but we need .m2
        let name = name.to_lowercase().replace(".mdx", ".m2");
        if name.to_lowercase().contains("emitter") ||
            name.to_uppercase().contains("GENERALDIRTYPLATE01") || name.to_uppercase().contains("GENERALCANDELABRA01")
            || name.to_uppercase().contains("GOLDSHIREINN")
            || name.to_lowercase().contains("02") || name.to_lowercase().contains("03") || name.to_lowercase().contains("01") {
            continue;
        }

        let entry = load_m2_doodad(common, common2, &mut m2_cache, &name);
        let scale = Vec3::new(dad_ref.scale as f32 / 1024.0, dad_ref.scale as f32 / 1024.0, dad_ref.scale as f32 / 1024.0);
        let rotation = Quat::from_euler(EulerRot::YXZ, dad_ref.rotation.x.to_radians(), dad_ref.rotation.y.to_radians(), dad_ref.rotation.z.to_radians());
        let mut translation = from_vec(dad_ref.position);
        // MDDFS (TODO: MODF) uses a completely different coordinate system, so we need to fix up things.
        translation.x = -translation.x; // west is positive X!!
        std::mem::swap(&mut translation.z, &mut translation.y); // maybe needs inverting.

        // // relative to a corner of the map, but we want to have the center in mid (just alone for rotation etc)
        // translation.x -= 17066.0; // WOWDEV
        // translation.y -= 17066.0;
        dbg!(translation);

        let transform: Affine3A = Affine3A::from_scale_rotation_translation(scale, rotation, translation);
        render_list.push((transform, entry.clone()));
    }

    rendering::render(render_list, wmos.iter().map(|wmo| (&wmo.0, &wmo.1)), texture_map);
    Ok(())
}

/// Extracts the doodads (i.e. M2 models that have been placed into the world at a specific position) that are defined in the WMO Root
fn collect_dooads(common: &mut Archive, common2: &mut Archive, m2_cache: &mut HashMap<String, Rc<(M2Asset, Vec<M2SkinProfile>, Option<BlpImage>)>>, wmo: &WMORootAsset) -> Vec<(Affine3A, Rc<(M2Asset, Vec<M2SkinProfile>, Option<BlpImage>)>)> {
    let mut render_list: Vec<(Affine3A, Rc<(M2Asset, Vec<M2SkinProfile>, Option<BlpImage>)>)> = Vec::new();
    for mods in &wmo.mods.doodadSetList {
        let start = mods.startIndex as usize;
        let end = (mods.startIndex + mods.count) as usize;
        println!("Doodad Set: {} from {} to {}", mods.name, start, end);
        for modd in &wmo.modd.doodadDefList[start..end] {
            let idx = wmo.modn.doodadNameListLookup[&modd.nameIndex];
            let name = wmo.modn.doodadNameList[idx].as_str();

            // fix name: currently it ends with .mdx but we need .m2
            let name = name.replace(".MDX", ".m2");
            if name.to_lowercase().contains("emitter") ||
                name.to_uppercase().contains("GENERALDIRTYPLATE01") || name.to_uppercase().contains("GENERALCANDELABRA01")
                || name.to_uppercase().contains("GOLDSHIREINN")
                || name.to_lowercase().contains("02") || name.to_lowercase().contains("03") || name.to_lowercase().contains("01") {
                continue;
            }

            let entry = load_m2_doodad(common, common2, m2_cache, &name);
            let transform: Affine3A = Affine3A::from_scale_rotation_translation(Vec3::new(modd.scale, modd.scale, modd.scale), from_quat(modd.orientation), from_vec(modd.position));
            render_list.push((transform, entry.clone()));
        }
    }

    render_list
}

fn load_m2_doodad(common: &mut Archive, common2: &mut Archive, m2_cache: &mut HashMap<String, Rc<(M2Asset, Vec<M2SkinProfile>, Option<BlpImage>)>>, name: &String) -> Rc<(M2Asset, Vec<M2SkinProfile>, Option<BlpImage>)> {
    let entry = m2_cache.entry(name.clone()).or_insert_with(|| {
        //let m2_file = common2.open_file(&name).unwrap_or_else(|_| panic!("File {} missing!", name));
        let mut m2_file = io::mpq::loader::read_mpq_file_into_cursor(common2, &name).unwrap();
        let m2_asset = M2Reader::parse_asset(&mut m2_file).unwrap();
        // In theory, we could investigate the number of LoD Levels, but we will just use "0"
        let mut skin_file = io::mpq::loader::read_mpq_file_into_cursor(common2, &name.replace(".m2", "00.skin")).unwrap();
        let skin = M2Reader::parse_skin_profile(&mut skin_file).unwrap();

        let mut blp_opt = None;
        if !m2_asset.textures.is_empty() && !name.eq("WORLD\\AZEROTH\\ELWYNN\\PASSIVEDOODADS\\TREES\\ELWYNNTREEMID01.m2") {
            blp_opt = load_blp_from_mpq(common, &m2_asset.textures[0].filename);
        }

        Rc::new((m2_asset, vec![skin], blp_opt))
    });
    entry.clone()
}

fn load_wmo_groups(common2: &mut Archive, wmo: &WMORootAsset, path: &str) -> Vec<WMOGroupAsset> {
    for group in &wmo.mogi.groupInfoList {
        if group.nameoffset != -1
        {
            let offset = wmo.mogn.offset_lookup[&(group.nameoffset as u32)];
            dbg!(&wmo.mogn.groupNameList[offset]);
        }
    }

    let mut group_list = Vec::new();
    for x in 0..wmo.mohd.nGroups {
        let cursor = &mut io::mpq::loader::read_mpq_file_into_cursor(common2,
                                                                       &format!("{}_{:0>3}.wmo", path, x)).unwrap();
        let group = WMOReader::parse_group(cursor).unwrap();
        group_list.push(group);
    }

    group_list
}

fn main_simple_m2(common: &mut Archive, expansion: &mut Archive) -> Result<(), anyhow::Error> {
    // This method demonstrates very simple m2 rendering (not in the context of wmos or adts).

    //debug_dump_blp(&mut expansion, r"WORLD\EXPANSION01\DOODADS\GENERIC\BLOODELF\BENCHES\BE_BENCH_01.BLP");
    //let m2_path = "World\\EXPANSION01\\DOODADS\\GENERIC\\BLOODELF\\Chairs\\BE_Chair01.m2";
    //let skin_path =  r"World\EXPANSION01\DOODADS\GENERIC\BLOODELF\Chairs\BE_Chair0100.skin";
    //let tex_path = r"WORLD\EXPANSION01\DOODADS\GENERIC\BLOODELF\BENCHES\BE_BENCH_01.BLP";

    let m2_path = r"Creature\talbuk\Talbuk.m2";
    let skin_path = r"Creature\talbuk\Talbuk00.skin";
    let tex_path = r"Creature\talbuk\TalbukSkinBrown.blp"; // common.mpq!

    let m2 = M2Reader::parse_asset(&mut io::mpq::loader::read_mpq_file_into_cursor(expansion, m2_path)?)?;
    let skin = M2Reader::parse_skin_profile(&mut io::mpq::loader::read_mpq_file_into_cursor(expansion, skin_path)?)?;
    let blp_opt = load_blp_from_mpq(common, tex_path);

    // Note: This API is already a bad monstrosity, it WILL go, but it makes prototyping easier.
    rendering::render(vec![(Affine3A::from_translation(Vec3::new(0.0, 0.0, 0.0)),
                            Rc::new((m2, vec![skin], blp_opt)))], vec![],
                      HashMap::new());
    Ok(())
}

fn main_simple_wmo(common: &mut Archive, common2: &mut Archive) -> Result<(), anyhow::Error> {
    // This method demonstrates very simple wmo rendering (not in the context of adts).
    //let wmo_path = r"World\wmo\Dungeon\AZ_Subway\Subway";
    //let wmo_path = r"World\wmo\Azeroth\Buildings\GoldshireInn\GoldshireInn"; <-- broken due to files loading error
    //let wmo_path = r"World\wmo\Azeroth\Buildings\GriffonAviary\GriffonAviary"; <-- broken, textures not located, skipping them
    let wmo_path = r"World\wmo\Azeroth\Buildings\GoldshireInn\GoldshireInn_closed";

    let mut wmo: WMORootAsset = WMOReader::parse_root(&mut io::mpq::loader::read_mpq_file_into_cursor(common2, &format!("{}.wmo", wmo_path))?)?;

    // TODO: m2_cache should become an implementation detail of the struct that provides collect_doodads?
    // TODO: collect_doodads shouldn't cotain the cache loading logic anyway.
    let mut m2_cache: HashMap<String, Rc<(M2Asset, Vec<M2SkinProfile>, Option<BlpImage>)>> = HashMap::new();

    let dooads = collect_dooads(common, common2, &mut m2_cache, &mut wmo);
    let group_list = load_wmo_groups(common2, &wmo, wmo_path);
    let textures = wmo.momt.materialList.iter()
        .map(|tex| wmo.motx.textureNameList[wmo.motx.offsets[&tex.texture_1]].clone())
        .collect_vec();

    let mut texture_map = HashMap::new();
    for texture in textures {
        let blp = load_blp_from_mpq(common, &texture).expect("Texture loading error");
        texture_map.insert(texture, blp);
    }

    let mut wmos = Vec::new();
    wmos.push((wmo, group_list));

    // Note: This API is already a bad monstrosity, it WILL go, but it makes prototyping easier.
    rendering::render(dooads, wmos.iter().map(|wmo| (&wmo.0, &wmo.1)), texture_map);
    Ok(())
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
