use std::collections::HashMap;
use std::io::Cursor;
use std::ops::DerefMut;
use std::sync::{Arc, RwLock};
use std::time::Instant;

use glam::{Affine3A, Vec3};
use image_blp::BlpImage;
use itertools::Itertools;
use log::{error, info, trace, warn};

use sargerust_files::adt::reader::ADTReader;
use sargerust_files::adt::types::ADTAsset;
use sargerust_files::wdt::reader::WDTReader;
use sargerust_files::wdt::types::WDTAsset;

use crate::io::common::loader::RawAssetLoader;
use crate::io::mpq::loader::MPQLoader;
use crate::rendering::asset_graph::m2_generator::M2Generator;
use crate::rendering::asset_graph::nodes::adt_node::{ADTNode, DoodadReference, IRTexture, M2Node};
use crate::rendering::asset_graph::resolver::Resolver;
use crate::rendering::common::coordinate_systems;
use crate::rendering::common::highlevel_types::{PlaceableDoodad, PlacedDoodad};
use crate::rendering::common::types::{Material, Mesh, MeshWithLod};
use crate::rendering::importer::adt_importer::ADTImporter;
use crate::transform_for_doodad_ref;

pub struct MapManager {
    mpq_loader: Arc<MPQLoader>,
    pub current_map: Option<(String, WDTAsset)>,
    pub loaded_tiles: HashMap<
        (u8, u8),
        (
            Arc<ADTAsset>,
            /* Terrain */ Vec<(Vec3, Mesh)>,
            Vec<PlacedDoodad>,
            /* WMO */ Vec<(Affine3A, Vec<(MeshWithLod, Vec<Material>)>)>,
            HashMap<String, BlpImage>,
        ),
    >,
    pub tile_graph: HashMap<(u8, u8), Arc<ADTNode>>,
    pub m2_resolver: Resolver<M2Generator, M2Node>,
    pub tex_resolver: Resolver<M2Generator, RwLock<Option<IRTexture>>>, /* failably */
}

impl MapManager {
    pub fn new(mpq_loader: Arc<MPQLoader>) -> Self {
        Self {
            mpq_loader: mpq_loader.clone(),
            current_map: None,
            loaded_tiles: HashMap::new(),
            tile_graph: HashMap::new(),
            m2_resolver: Resolver::new(M2Generator::new(mpq_loader.clone())),
            tex_resolver: Resolver::new(M2Generator::new(mpq_loader)),
        }
    }

    pub fn preload_map(&mut self, map: String, position: Vec3, orientation: f32) {
        let now = Instant::now();
        info!("Loading map {} @ {}", map, position);
        let wdt_buf = self
            .mpq_loader
            .as_ref()
            .load_raw_owned(&format!("world\\maps\\{}\\{}.wdt", map, map));
        let wdt =
            WDTReader::parse_asset(&mut Cursor::new(wdt_buf.expect("Cannot load map wdt"))).expect("Error parsing WDT");

        let chunk_coords_pos = coordinate_systems::adt_world_to_tiles(position);
        // TODO: We expect the result to be (row, column), but for some reason, it seems to be (column, row)

        for x in /*-1i8..2*/ 0..1 {
            for y in /*-1i8..2*/ 0..1 {
                let chunk_coords = (
                    (chunk_coords_pos.0 as i8 + x) as u8,
                    (chunk_coords_pos.1 as i8 + y) as u8,
                );

                if wdt.has_chunk(chunk_coords.1, chunk_coords.0) {
                    let adt_buf = self.mpq_loader.as_ref().load_raw_owned(&format!(
                        "world\\maps\\{}\\{}_{}_{}.adt",
                        map, map, chunk_coords.1, chunk_coords.0
                    ));
                    let adt = ADTReader::parse_asset(&mut Cursor::new(adt_buf.expect("Cannot load map adt")))
                        .expect("Error parsing ADT");
                    trace!("Loaded tile {}_{}_{}", map, chunk_coords.1, chunk_coords.0);

                    let mut render_list = Vec::new();
                    let mut wmos = Vec::new();

                    let terrain_chunk = self
                        .handle_adt_lazy(&adt, &mut render_list, &mut wmos)
                        .unwrap();

                    let graph = ADTNode {
                        terrain: terrain_chunk
                            .into_iter()
                            .map(|chunk| (chunk.0.into(), RwLock::new(chunk.1.into())))
                            .collect_vec(),
                        doodads: render_list,
                    };

                    self.tile_graph.insert(chunk_coords, Arc::new(graph));
                } else {
                    error!("We load into the world on unmapped terrain?!");
                }
            }
        }

        self.current_map = Some((map, wdt));
        warn!("Loading took {}ms", now.elapsed().as_millis());
        // ADT file is map_x_y.adt. I think x are rows and ys are columns.
    }

    fn handle_adt_lazy(
        &self,
        adt: &ADTAsset,
        render_list: &mut Vec<DoodadReference>,
        wmos: &mut Vec<(Affine3A, Vec<(MeshWithLod, Vec<Material>)>)>,
    ) -> Result<Vec<(Vec3, Mesh)>, anyhow::Error> {
        for dad_ref in &adt.mddf.doodadDefs {
            let name = &adt.mmdx.filenames[*adt
                .mmdx
                .offsets
                .get(&adt.mmid.mmdx_offsets[dad_ref.nameId as usize])
                .unwrap()];
            //trace!("M2 {} has been referenced from ADT", name);

            // fix name: currently it ends with .mdx but we need .m2
            let name = name
                .to_lowercase()
                .replace(".mdx", ".m2")
                .replace(".mdl", ".m2");

            // TODO: this (and the string replace) could also happen on consumer level, where the ADTNode is built
            if name.to_lowercase().contains("emitter") {
                continue;
            }

            render_list.push(DoodadReference::new(
                transform_for_doodad_ref(dad_ref).into(),
                name,
            ));
        }

        // for wmo_ref in adt.modf.mapObjDefs.iter() {
        //     let name = &adt.mwmo.filenames[*adt
        //         .mwmo
        //         .offsets
        //         .get(&adt.mwid.mwmo_offsets[wmo_ref.nameId as usize])
        //         .unwrap()];
        //     trace!("WMO {} has been referenced from ADT", name);
        //
        //     if name.ends_with("STORMWIND.WMO") {
        //         continue; // TODO: Temporary performance optimization
        //     }
        //
        //     let loaded = WMOLoader::load(loader, name)?;
        //     // TODO: currently, only WMO makes use of texture names, M2s load their textures in load_m2_doodad (when the doodad becomes placeable).
        //     let textures = loaded
        //         .loaded_groups
        //         .iter()
        //         .flat_map(|(_, mats)| mats)
        //         .filter_map(|mat| match &mat.albedo {
        //             AlbedoType::TextureWithName(tex_name) => Some(tex_name.clone()),
        //             _ => None,
        //         })
        //         .collect_vec();
        //
        //     for texture in textures {
        //         if !texture_map.contains_key(&texture) {
        //             let blp = BLPLoader::load_blp_from_ldr(loader, &texture).expect("Texture loading error");
        //             texture_map.insert(texture, blp);
        //         }
        //     }
        //
        //     let transform = transform_for_wmo_ref(wmo_ref);
        //     for dad in loaded.doodads {
        //         // NOTE: Here we loose the relationship between DAD and wmo, that is required for parenting.
        //         // Since rend3 does not have a scenegraph, we "fake" the parenting for now.
        //         // Also we need to resolve m2 references.
        //         render_list.push(PlacedDoodad {
        //             transform: transform * dad.transform,
        //             m2: load_m2_doodad(loader, m2_cache, &dad.m2_ref),
        //         });
        //     }
        //
        //     wmos.push((transform, loaded.loaded_groups));
        // }

        let mut terrain_chunk = vec![];
        for mcnk in &adt.mcnks {
            terrain_chunk.push(ADTImporter::create_mesh(mcnk, false)?);
        }

        // TODO: Resolving would be enqueued as futures on a tokio runtime
        for dad in render_list {
            let result = self
                .m2_resolver
                .resolve(dad.reference.reference_str.clone());

            // TODO: yet another enqueue for tokio
            for tex_reference in result.tex_reference.iter() {
                let result_tex = self
                    .tex_resolver
                    .resolve(tex_reference.reference_str.clone());

                let mut ref_wlock = tex_reference
                    .reference
                    .write()
                    .expect("texture reference write lock");

                *ref_wlock.deref_mut() = Some(result_tex);
            }

            let mut write_lock = dad
                .reference
                .reference
                .write()
                .expect("Write lock on doodad reference");

            *write_lock.deref_mut() = Some(result);
        }

        Ok(terrain_chunk)
    }
}
