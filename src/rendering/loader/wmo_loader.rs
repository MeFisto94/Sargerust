use glam::{Affine3A, Quat, Vec3};
use log::{debug};
use sargerust_files::wmo::reader::WMOReader;
use sargerust_files::wmo::types::WMORootAsset;
use crate::io::common::loader::RawAssetLoader;
use crate::io::mpq::loader::MPQLoader;
use crate::rendering::common::highlevel_types::{PlaceableDoodad, PlaceableWMO};
use crate::rendering::importer::wmo_importer::WMOGroupImporter;

pub struct WMOLoader {

}

impl WMOLoader {
    pub fn load(loader: &MPQLoader, wmo_path: &str) -> Result<PlaceableWMO, anyhow::Error> { // TODO: thiserror
        let wmo: WMORootAsset = WMOReader::parse_root(&mut std::io::Cursor::new(loader.load_raw_owned(wmo_path).unwrap()))?;
        let doodads = WMOLoader::collect_dooads_for_wmo_root(&wmo);
        let group_list = WMOGroupImporter::load_wmo_groups(loader, &wmo, wmo_path.to_uppercase().trim_end_matches(".WMO"));

        Ok(PlaceableWMO {
            doodads,
            loaded_groups: group_list
        })
    }

    /// Extracts the doodads (i.e. M2 models that have been placed into the world at a specific position) that are defined in the WMO Root
    pub fn collect_dooads_for_wmo_root(wmo: &WMORootAsset) -> Vec<PlaceableDoodad> {
        let mut render_list = Vec::new();
        for mods in &wmo.mods.doodadSetList {
            let start = mods.startIndex as usize;
            let end = (mods.startIndex + mods.count) as usize;
            debug!("Doodad Set: {} from {} to {}", mods.name, start, end);
            // TODO: at some point we need logic to selectively filter dooddad sets.
            for modd in &wmo.modd.doodadDefList[start..end] {
                let idx = wmo.modn.doodadNameListLookup[&modd.nameIndex];
                let name = wmo.modn.doodadNameList[idx].as_str();

                // fix name: currently it ends with .mdx but we need .m2
                let name = name.replace(".MDX", ".m2").replace(".MDL", ".m2");
                if name.to_lowercase().contains("emitter") {
                    continue;
                }

                let scale = Vec3::new(modd.scale, modd.scale, modd.scale);
                let rotation = Quat::from_xyzw(modd.orientation.x, modd.orientation.y, modd.orientation.z, modd.orientation.w);
                let translation = Vec3::new(modd.position.x, modd.position.y, modd.position.z);

                let transform: Affine3A = Affine3A::from_scale_rotation_translation(scale, rotation, translation);
                render_list.push(PlaceableDoodad {
                    transform,
                    m2_ref: name
                });
            }
        }

        render_list
    }
}