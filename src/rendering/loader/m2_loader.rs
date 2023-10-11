use crate::io::common::loader::RawAssetLoader;
use crate::io::mpq::loader::MPQLoader;
use crate::rendering::asset_graph::nodes::adt_node::{IRTextureReference, M2Node};
use crate::rendering::common::types::{Material, Mesh};
use crate::rendering::importer::m2_importer::M2Importer;
use crate::rendering::loader::blp_loader::BLPLoader;
use image_blp::BlpImage;
use itertools::Itertools;
use sargerust_files::m2::reader::M2Reader;

#[derive(Debug, Clone)]
pub struct LoadedM2 {
    pub mesh: Mesh,
    pub material: Material,

    // TODO: The Material will probably contain texture reference, but at least texture paths, so they can be loaded independently.
    pub blp_opt: Option<BlpImage>,
}

#[derive(Debug)]
pub struct LoadedM2Graph {
    pub mesh: Mesh,
    pub material: Material,
    pub textures: Vec<IRTextureReference>,
}

pub struct M2Loader {}

impl M2Loader {
    #[deprecated]
    pub fn load_no_lod(loader: &MPQLoader, name: &str) -> LoadedM2 {
        let m2_asset = M2Reader::parse_asset(&mut std::io::Cursor::new(
            loader.load_raw_owned(name).unwrap(),
        ))
        .unwrap();
        // In theory, we could investigate the number of LoD Levels, but we will just use "0"
        let mut skin_file = std::io::Cursor::new(
            loader
                .load_raw_owned(&name.replace(".m2", "00.skin"))
                .unwrap(),
        );
        let skin = M2Reader::parse_skin_profile(&mut skin_file).unwrap();

        let mut blp_opt = None;
        if !m2_asset.textures.is_empty() {
            blp_opt = BLPLoader::load_blp_from_ldr(loader, &m2_asset.textures[0].filename);
        }

        let mesh = M2Importer::create_mesh(&m2_asset, &skin);
        let material = M2Importer::create_material(&blp_opt); // TODO: the texture should be intrinsic to the material.

        LoadedM2 {
            mesh,
            material,
            blp_opt,
        }
    }

    // TODO: this could immediately return a M2Node as all that it additionally does is some .into()
    pub fn load_no_lod_for_graph(loader: &MPQLoader, name: &str) -> LoadedM2Graph {
        let m2_asset = M2Reader::parse_asset(&mut std::io::Cursor::new(
            loader.load_raw_owned(name).unwrap(),
        ))
        .unwrap();
        // In theory, we could investigate the number of LoD Levels, but we will just use "0"
        let mut skin_file = std::io::Cursor::new(
            loader
                .load_raw_owned(&name.replace(".m2", "00.skin"))
                .unwrap(),
        );

        let skin = M2Reader::parse_skin_profile(&mut skin_file).unwrap();
        let mesh = M2Importer::create_mesh(&m2_asset, &skin);

        let textures: Vec<IRTextureReference> = m2_asset
            .textures
            .into_iter()
            .map(|tex| tex.into())
            .collect();

        let material = M2Importer::create_material_texname(&textures.get(0).map(|tex| tex.reference_str.clone()));

        LoadedM2Graph {
            mesh,
            material,
            textures,
        }
    }
}
