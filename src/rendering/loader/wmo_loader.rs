use crate::io::common::loader::RawAssetLoader;
use crate::io::mpq::loader::MPQLoader;
use crate::rendering::asset_graph::nodes::adt_node::{DoodadReference, NodeReference, WMOGroupNode, WMONode};
use crate::rendering::common::highlevel_types::PlaceableDoodad;
use crate::rendering::common::types::{AlbedoType, Material, TransparencyType};
use crate::rendering::rend3_backend::IRTextureReference;
use arc_swap::ArcSwapOption;
use glam::{Affine3A, Quat, Vec3, Vec4};
use log::debug;
use sargerust_files::wmo::reader::WMOReader;
use sargerust_files::wmo::types::WMORootAsset;
use std::sync::{Arc, RwLock};

pub struct WMOLoader {}

impl WMOLoader {
    pub fn load_graph(loader: &MPQLoader, wmo_path: &str) -> Result<WMONode, anyhow::Error> {
        // TODO: thiserror
        let wmo: WMORootAsset = WMOReader::parse_root(&mut std::io::Cursor::new(
            loader.load_raw_owned(wmo_path).unwrap(),
        ))?;

        // TODO: doodad sets?
        let mut doodads = Vec::new();
        for dad in WMOLoader::collect_doodads_for_wmo_root(&wmo) {
            doodads.push(Arc::new(DoodadReference::new(
                dad.transform.into(),
                dad.m2_ref,
            )));
        }

        let mut subgroups = Vec::with_capacity(wmo.mohd.nGroups as usize);
        let mut materials = Vec::with_capacity(wmo.momt.materialList.len());
        let mut tex_references = Vec::with_capacity(wmo.momt.materialList.len());

        for material in &wmo.momt.materialList {
            // texture_1 defaults to "createcrappygreentexture.blp" in the original client
            let texname_1 = wmo.motx.textureNameList[wmo.motx.offsets[&material.texture_1]].clone();
            let has_tex = !texname_1.trim_end_matches('\0').is_empty();

            // TODO: texture_2

            let mat = Material {
                albedo: match has_tex {
                    false => AlbedoType::Value(Vec4::new(
                        material.diffColor.r as f32 / 255.0,
                        material.diffColor.g as f32 / 255.0,
                        material.diffColor.b as f32 / 255.0,
                        material.diffColor.a as f32 / 255.0,
                    )),
                    true => AlbedoType::TextureWithName(texname_1.clone()),
                },
                transparency: match material.blendMode {
                    0 => TransparencyType::Opaque,
                    1 => TransparencyType::Cutout { cutout: 0.5 }, // 0.5 is just a random "guess"
                    _ => TransparencyType::Opaque,
                },
            };

            materials.push(RwLock::new(mat.into()));

            if has_tex {
                // TODO: Since everything is behind RwLocks anyway, can we maybe construct TexReferences to be Arc?
                //  Then we could share them (e.g when multiple materials reference the same texture), but the gain
                //  is rather minor, just some locking and resolving.
                tex_references.push(Arc::new(IRTextureReference {
                    reference_str: texname_1,
                    reference: ArcSwapOption::empty(),
                }))
            }
        }

        let path_upper = wmo_path.to_uppercase();
        let path = path_upper.trim_end_matches(".WMO");

        for x in 0..wmo.mohd.nGroups {
            subgroups.push(Arc::new(NodeReference::<WMOGroupNode> {
                reference_str: format!("{}_{:0>3}.wmo", path, x),
                reference: Default::default(),
            }));
        }

        Ok(WMONode {
            doodads,
            subgroups,
            materials,
            tex_references,
        })
    }

    /// Extracts the doodads (i.e. M2 models that have been placed into the world at a specific position) that are defined in the WMO Root
    pub fn collect_doodads_for_wmo_root(wmo: &WMORootAsset) -> Vec<PlaceableDoodad> {
        let mut render_list = Vec::new();
        for mods in &wmo.mods.doodadSetList {
            let start = mods.startIndex as usize;
            let end = (mods.startIndex + mods.count) as usize;
            debug!("Doodad Set: {} from {} to {}", &mods.name, start, end);
            // TODO: at some point we need logic to selectively filter dooddad sets.
            for modd in &wmo.modd.doodadDefList[start..end] {
                let idx = wmo.modn.doodadNameListLookup[&modd.nameIndex];
                let name = wmo.modn.doodadNameList[idx].as_str();

                // fix name: currently it ends with .mdx, but we need .m2
                let name = name.replace(".MDX", ".M2").replace(".MDL", ".M2");
                if name.to_lowercase().contains("emitter") {
                    continue;
                }

                let scale = Vec3::new(modd.scale, modd.scale, modd.scale);
                let rotation = Quat::from_xyzw(
                    modd.orientation.x,
                    modd.orientation.y,
                    modd.orientation.z,
                    modd.orientation.w,
                );
                let translation = Vec3::new(modd.position.x, modd.position.y, modd.position.z);

                let transform: Affine3A = Affine3A::from_scale_rotation_translation(scale, rotation, translation);
                render_list.push(PlaceableDoodad {
                    transform,
                    m2_ref: name,
                    set_name: mods.name.clone(),
                });
            }
        }

        render_list
    }
}
