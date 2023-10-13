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
use crate::rendering::asset_graph::nodes::adt_node::{
    ADTNode, DoodadReference, IRTexture, IRTextureReference, M2Node, NodeReference, WMOGroupNode, WMONode, WMOReference,
};
use crate::rendering::asset_graph::resolver::Resolver;
use crate::rendering::common::coordinate_systems;
use crate::rendering::common::highlevel_types::PlacedDoodad;
use crate::rendering::common::types::{AlbedoType, Material, Mesh, MeshWithLod};
use crate::rendering::importer::adt_importer::ADTImporter;
use crate::{transform_for_doodad_ref, transform_for_wmo_ref};

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
    pub wmo_resolver: Resolver<M2Generator, WMONode>,
    pub wmo_group_resolver: Resolver<M2Generator, WMOGroupNode>,
}

impl MapManager {
    pub fn new(mpq_loader: Arc<MPQLoader>) -> Self {
        Self {
            mpq_loader: mpq_loader.clone(),
            current_map: None,
            loaded_tiles: HashMap::new(),
            tile_graph: HashMap::new(),
            m2_resolver: Resolver::new(M2Generator::new(mpq_loader.clone())),
            tex_resolver: Resolver::new(M2Generator::new(mpq_loader.clone())),
            wmo_resolver: Resolver::new(M2Generator::new(mpq_loader.clone())),
            wmo_group_resolver: Resolver::new(M2Generator::new(mpq_loader)),
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
                        wmos,
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
        direct_doodad_refs: &mut Vec<DoodadReference>,
        wmos: &mut Vec<WMOReference>,
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

            direct_doodad_refs.push(DoodadReference::new(
                transform_for_doodad_ref(dad_ref).into(),
                name,
            ));
        }

        for &wmo_ref in adt.modf.mapObjDefs.iter() {
            let name = &adt.mwmo.filenames[*adt
                .mwmo
                .offsets
                .get(&adt.mwid.mwmo_offsets[wmo_ref.nameId as usize])
                .unwrap()];
            trace!("WMO {} has been referenced from ADT", name);

            if name.ends_with("STORMWIND.WMO") {
                continue; // TODO: Temporary performance optimization
            }

            let transform = transform_for_wmo_ref(&wmo_ref);
            wmos.push(WMOReference::new(wmo_ref, transform, name.to_owned()));
        }

        let mut terrain_chunk = vec![];
        for mcnk in &adt.mcnks {
            terrain_chunk.push(ADTImporter::create_mesh(mcnk, false)?);
        }

        // TODO: Resolving would be enqueued as futures on a tokio runtime
        // TODO: Resolving should be a matter of the rendering app, not this code here? But then their code relies on things being preloaded?

        let mut wmo_arcs = Vec::new(); // used to load doodads after all wmos have been resolved
        for wmo in wmos {
            let result = self
                .wmo_resolver
                .resolve(wmo.reference.reference_str.clone());

            // TODO: should we resolve WMO Groups right here or rather after all WMOs are resolved? Technically groups could even be lazily resolved?
            for sub_group in &result.subgroups {
                let group_result = self
                    .wmo_group_resolver
                    .resolve(sub_group.reference_str.to_string());

                let mut write_lock_group = sub_group
                    .reference
                    .write()
                    .expect("Write lock on sub group reference");

                *write_lock_group.deref_mut() = Some(group_result);
            }

            // TODO: optimize. Since all materials and textures reside on the WMO level, they are loaded, even when the subgroup that needs them isn't.
            self.resolve_tex_reference(&result.tex_references);

            wmo_arcs.push(result.clone());
            let mut write_lock = wmo
                .reference
                .reference
                .write()
                .expect("Write lock on wmo reference");

            *write_lock.deref_mut() = Some(result);
        }

        let wmo_dads = wmo_arcs.iter().flat_map(|wmo| &wmo.doodads).collect_vec();

        for dad in direct_doodad_refs.iter().interleave(wmo_dads) {
            // TODO: PERF: get rid of duplicated references here, make DoodadReference#reference an Arc and cache them at least adt wide.
            let result = self
                .m2_resolver
                .resolve(dad.reference.reference_str.clone());

            // TODO: yet another enqueue for tokio
            self.resolve_tex_reference(&result.tex_reference);

            let mut write_lock = dad
                .reference
                .reference
                .write()
                .expect("Write lock on doodad reference");

            *write_lock.deref_mut() = Some(result);
        }

        Ok(terrain_chunk)
    }

    fn resolve_tex_reference(&self, references: &Vec<IRTextureReference>) {
        for tex_reference in references.iter() {
            let result_tex = self
                .tex_resolver
                .resolve(tex_reference.reference_str.clone());

            let mut ref_wlock = tex_reference
                .reference
                .write()
                .expect("texture reference write lock");

            *ref_wlock.deref_mut() = Some(result_tex);
        }
    }
}
