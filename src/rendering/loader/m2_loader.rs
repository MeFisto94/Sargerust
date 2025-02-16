use std::sync::Arc;

use crate::io::common::loader::RawAssetLoader;
use crate::io::mpq::loader::MPQLoader;
use crate::rendering::common::types::Mesh;
use crate::rendering::importer::m2_importer::{M2Importer, M2Material};
use crate::rendering::rend3_backend::IRTextureReference;
use itertools::Itertools;
use sargerust_files::m2::reader::M2Reader;
use sargerust_files::m2::types::{M2Texture, M2TextureType};

#[derive(Debug)]
pub struct M2MeshAndMaterial {
    pub mesh: Mesh,
    pub material: M2Material,
    pub geoset_index: u16,
}

#[derive(Debug)]
pub struct LoadedM2Graph {
    pub mesh_and_material: Vec<M2MeshAndMaterial>,
    pub textures: Vec<Arc<IRTextureReference>>,
    pub dynamic_textures: Vec<M2Texture>, // TODO: This can't be a reference sadly.
}

pub struct M2Loader {}

impl M2Loader {
    // TODO: this could immediately return a M2Node as all that it additionally does is some .into()
    pub fn load_no_lod_for_graph(loader: &MPQLoader, name: &str) -> LoadedM2Graph {
        let m2_asset = M2Reader::parse_asset(&mut std::io::Cursor::new(
            loader.load_raw_owned(name).unwrap(),
        ))
        .unwrap();
        // In theory, we could investigate the number of LoD Levels, but we will just use "0"
        let mut skin_file = std::io::Cursor::new(
            loader
                .load_raw_owned(&name.replace(".M2", "00.skin"))
                .unwrap(),
        );

        let skin = M2Reader::parse_skin_profile(&mut skin_file).unwrap();

        // TODO: We can re-use meshes between batches if we put them in Arcs _and_ ensure that their usage doesn't copy
        //  them but uses the same backend handle, which is the harder part.

        let mesh_and_material = skin
            .batches
            .iter()
            .map(|batch| {
                // TODO: panic to thiserror?
                let sub_mesh = skin
                    .submeshes
                    .get(batch.skinSectionIndex as usize)
                    .unwrap_or_else(|| {
                        panic!(
                            "Batch is linking an invalid skinSectionIndex {}",
                            batch.skinSectionIndex
                        )
                    });

                let mesh = M2Importer::create_mesh(&m2_asset, &skin, &sub_mesh);
                let material = M2Importer::create_m2_material(&m2_asset, batch);
                let geoset_index = sub_mesh.skinSectionId;

                M2MeshAndMaterial {
                    mesh,
                    material,
                    geoset_index,
                }
            })
            .collect_vec();

        let textures: Vec<Arc<IRTextureReference>> = m2_asset
            .textures
            .iter()
            .filter(|tex| tex.texture_type == M2TextureType::None)
            .map(|tex| Arc::new(tex.clone().into())) // TODO: This into should support references too
            .collect();

        let dynamic_textures = m2_asset
            .textures
            .into_iter()
            .filter(|tex| tex.texture_type != M2TextureType::None)
            .collect();

        LoadedM2Graph {
            mesh_and_material,
            textures,
            dynamic_textures,
        }
    }
}
