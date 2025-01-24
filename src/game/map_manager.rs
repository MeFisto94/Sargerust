use std::collections::HashMap;
use std::io::Cursor;
use std::ops::DerefMut;
use std::sync::{Arc, RwLock};
use std::time::Instant;

use glam::{Vec3, Vec3A};
use itertools::Itertools;
use log::{error, info, trace, warn};
use tokio::runtime::{Builder, Handle, Runtime};
use tokio::task::JoinSet;

use sargerust_files::adt::reader::ADTReader;
use sargerust_files::adt::types::ADTAsset;
use sargerust_files::wdt::reader::WDTReader;
use sargerust_files::wdt::types::{MPHDChunk, SMMapObjDef, WDTAsset};

use crate::io::common::loader::RawAssetLoader;
use crate::io::mpq::loader::MPQLoader;
use crate::rendering::asset_graph::m2_generator::M2Generator;
use crate::rendering::asset_graph::nodes::adt_node::{
    ADTNode, DoodadReference, IRObject, IRTexture, IRTextureReference, M2Node, TerrainTile, WMOGroupNode, WMONode,
    WMOReference,
};
use crate::rendering::asset_graph::resolver::Resolver;
use crate::rendering::common::coordinate_systems;
use crate::rendering::common::special_types::TerrainTextureLayerRend3;
use crate::rendering::importer::adt_importer::ADTImporter;
use crate::{transform_for_doodad_ref, transform_for_wmo_ref};

pub struct MapManager {
    runtime: Runtime,
    mpq_loader: Arc<MPQLoader>,
    pub current_map: Option<(String, WDTAsset)>,
    pub tile_graph: HashMap<(u8, u8), Arc<ADTNode>>,
    pub m2_resolver: Arc<Resolver<M2Generator, M2Node>>,
    pub tex_resolver: Arc<Resolver<M2Generator, RwLock<Option<IRTexture>>>>, /* failably */
    pub wmo_resolver: Arc<Resolver<M2Generator, WMONode>>,
    pub wmo_group_resolver: Arc<Resolver<M2Generator, WMOGroupNode>>,
}

impl MapManager {
    pub fn new(mpq_loader: Arc<MPQLoader>) -> Self {
        Self {
            mpq_loader: mpq_loader.clone(),
            current_map: None,
            tile_graph: HashMap::new(),
            // TODO: work on sharing the M2Generator.
            m2_resolver: Arc::new(Resolver::new(M2Generator::new(mpq_loader.clone()))),
            tex_resolver: Arc::new(Resolver::new(M2Generator::new(mpq_loader.clone()))),
            wmo_resolver: Arc::new(Resolver::new(M2Generator::new(mpq_loader.clone()))),
            wmo_group_resolver: Arc::new(Resolver::new(M2Generator::new(mpq_loader.clone()))),
            runtime: Builder::new_multi_thread()
                .build()
                .expect("Tokio Runtime to be built"),
        }
    }

    pub fn update_camera(&mut self, position: Vec3A) {
        if self.current_map.is_none() {
            return;
        }

        let coords = coordinate_systems::adt_world_to_tiles(position.into());
        if self.tile_graph.contains_key(&coords) {
            return;
        }

        // TODO: unloading and proper range based checks once the API around here stabilizesd
        self.try_load_chunk(&coords);
    }

    // TODO: I am not sure if the whole preloading shouldn't be the responsibility of the render thread and if we as src\game should at best care about building the graph.
    pub fn preload_map(
        &mut self,
        map: String,
        position: Vec3,
        _orientation: f32, /* TODO: use for foveated preloading */
    ) {
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
                    self.load_chunk(&map, &chunk_coords, &wdt.mphd);
                } else {
                    error!("We load into the world on unmapped terrain?!");
                }
            }
        }

        self.current_map = Some((map, wdt));
        warn!("Loading took {}ms", now.elapsed().as_millis());
        // ADT file is map_x_y.adt. I think x are rows and ys are columns.
    }

    fn try_load_chunk(&mut self, coords: &(u8, u8)) -> bool {
        if let Some((map, wdt)) = self.current_map.as_ref() {
            let mphd = wdt.mphd;
            if wdt.has_chunk(coords.1, coords.0) {
                self.load_chunk(&map.clone(), coords, &mphd);
                return true;
            }
        }

        false
    }
    fn load_chunk(&mut self, map: &String, chunk_coords: &(u8, u8), mphd: &MPHDChunk) {
        let adt_buf = self.mpq_loader.as_ref().load_raw_owned(&format!(
            "world\\maps\\{}\\{}_{}_{}.adt",
            map, map, chunk_coords.1, chunk_coords.0
        ));
        let adt =
            ADTReader::parse_asset(&mut Cursor::new(adt_buf.expect("Cannot load map adt"))).expect("Error parsing ADT");
        trace!("Loaded tile {}_{}_{}", map, chunk_coords.1, chunk_coords.0);
        let graph = self.handle_adt_lazy(&adt, mphd).unwrap();
        self.tile_graph.insert(*chunk_coords, Arc::new(graph));
    }

    fn handle_adt_lazy(&self, adt: &ADTAsset, mphd: &MPHDChunk) -> Result<ADTNode, anyhow::Error> {
        let mut direct_doodad_refs = Vec::new();
        let mut wmos = Vec::new();

        for dad_ref in &adt.mddf.doodadDefs {
            let name = &adt.mmdx.filenames[*adt
                .mmdx
                .offsets
                .get(&adt.mmid.mmdx_offsets[dad_ref.nameId as usize])
                .unwrap()];
            //trace!("M2 {} has been referenced from ADT", name);

            // fix name: currently it ends with .mdx, but we need .m2
            let name = name
                .to_lowercase()
                .replace(".mdx", ".m2")
                .replace(".mdl", ".m2");

            // TODO: this (and the string replace) could also happen on consumer level, where the ADTNode is built
            if name.to_lowercase().contains("emitter") {
                continue;
            }

            direct_doodad_refs.push(Arc::new(DoodadReference::new(
                transform_for_doodad_ref(dad_ref).into(),
                name,
            )));
        }

        for &wmo_ref in adt.modf.mapObjDefs.iter() {
            let name = &adt.mwmo.filenames[*adt
                .mwmo
                .offsets
                .get(&adt.mwid.mwmo_offsets[wmo_ref.nameId as usize])
                .unwrap()];
            //trace!("WMO {} has been referenced from ADT", name);

            if name.ends_with("STORMWIND.WMO") {
                continue; // TODO: Temporary performance optimization
            }

            if let Some(wmo_reference) = self.try_find_wmo_ref(&wmo_ref, name) {
                wmos.push(wmo_reference);
            } else {
                // TODO: There's a race condition from this line until this method terminates. And
                //  it even fails to find WMORefs already present in wmos, which is kinda a file fault anyway.
                let transform = transform_for_wmo_ref(&wmo_ref);
                wmos.push(Arc::new(WMOReference::new(
                    wmo_ref,
                    transform,
                    name.to_owned(),
                )));
            }
        }

        let mut set = JoinSet::new();

        let mut terrain_chunk = vec![];
        for mcnk in &adt.mcnks {
            let mesh = ADTImporter::create_mesh(mcnk, false, &adt.mtex, mphd)?;

            let texture_layers = mesh
                .2
                .into_iter()
                .map(|tref| {
                    let tex_ref = Arc::new(tref.texture_path.into());
                    let alpha = tref
                        .alpha_map
                        .map(|data| RwLock::new(IRObject { data, handle: None }));
                    TerrainTextureLayerRend3::new(tex_ref, alpha)
                })
                .collect_vec();

            let tile = TerrainTile {
                position: mesh.0.into(),
                mesh: RwLock::new(mesh.1.into()),
                object_handle: RwLock::new(None),
                texture_layers,
            };

            // TODO: This is a bit sketchy, why do we need to kick this off manually. Also think about the JoinSet again, this isn't exactly lazy then.
            let references = tile
                .texture_layers
                .iter()
                .map(|layer| layer.base_texture_ref.clone())
                .collect();
            Self::resolve_tex_reference(
                self.runtime.handle(),
                &mut set,
                self.tex_resolver.clone(),
                references,
            );
            terrain_chunk.push(tile);
        }

        // TODO: Resolving should be a matter of the rendering app, not this code here? But then their code relies on things being preloaded?
        for wmo in &wmos {
            if wmo
                .reference
                .reference
                .read()
                .expect("Reference ReadLock")
                .as_ref()
                .is_some()
            {
                // Apparently our reference was cloned and thus is pre-loaded.
                continue;
            }

            let result = self
                .wmo_resolver
                .resolve(wmo.reference.reference_str.clone());

            // TODO: should we resolve WMO Groups right here or rather after all WMOs are resolved? Technically groups could even be lazily resolved?
            for sub_group in &result.subgroups {
                let resolver = self.wmo_group_resolver.clone();
                let sub_group_cloned = sub_group.clone();
                set.spawn_blocking_on(
                    move || {
                        let group_result = resolver.resolve(sub_group_cloned.reference_str.to_string());

                        let mut write_lock_group = sub_group_cloned
                            .reference
                            .write()
                            .expect("Write lock on sub group reference");

                        *write_lock_group.deref_mut() = Some(group_result);
                    },
                    self.runtime.handle(),
                );
            }

            // TODO: optimize. Since all materials and textures reside on the WMO level, they are loaded, even when the subgroup that needs them isn't.
            Self::resolve_tex_reference(
                self.runtime.handle(),
                &mut set,
                self.tex_resolver.clone(),
                result.tex_references.clone(),
            );

            // Contrary to the previous use case description, we kick of wmo doodad loading before
            // all WMOs have been loaded, because it (may) improve performance by ensuring there's
            // always enough work to be done. Otherwise we could have two cores resolving the two
            // wmos and the rest of the application waits until it knows about doodads. Also, that
            // way we have a little less allocations. Another aspect here is that the resolvers are
            // a bottleneck that can lock. Thus it's better to distribute load between the resolvers
            // more than going in very synchronous and force-inserting data masses once per resolver.
            // To give an impact detail: When loading one northshire adt and still with resolver
            // having a RwLock<HashMap<_>>,summed thread wait time has gone down from 270s to 205s.
            // the tex_resolver is the most occupied resolver, which is no wonder since most textures
            // also stem from common.mpq thus blocking on loading as well.
            for dad in &result.doodads {
                let m2_resolver = self.m2_resolver.clone();
                let tex_resolver = self.tex_resolver.clone();

                Self::spawn_doodad_resolvers(
                    self.runtime.handle(),
                    &mut set,
                    dad.clone(),
                    m2_resolver,
                    tex_resolver,
                );
            }

            let mut write_lock = wmo
                .reference
                .reference
                .write()
                .expect("Write lock on wmo reference");

            *write_lock.deref_mut() = Some(result);
        }

        for dad in &direct_doodad_refs {
            let m2_resolver = self.m2_resolver.clone();
            let tex_resolver = self.tex_resolver.clone();

            Self::spawn_doodad_resolvers(
                self.runtime.handle(),
                &mut set,
                dad.clone(),
                m2_resolver,
                tex_resolver,
            );
        }

        // We need to poll the JoinSet
        self.runtime.spawn_blocking(move || {
            while let Some(result) = pollster::block_on(set.join_next()) {
                result.expect("Loading to be successful");
            }
        });

        Ok(ADTNode {
            terrain: terrain_chunk,
            doodads: direct_doodad_refs,
            wmos,
        })
    }

    fn try_find_wmo_ref(&self, needle: &SMMapObjDef, needle_str: &str) -> Option<Arc<WMOReference>> {
        self.tile_graph
            .values()
            .find_map(|graph| {
                graph.wmos.iter().find(|wmo| {
                    wmo.map_obj_def.uniqueId == needle.uniqueId && wmo.reference.reference_str.eq(needle_str)
                })
            })
            .cloned()
    }

    fn spawn_doodad_resolvers(
        handle: &Handle,
        set: &mut JoinSet<()>,
        dad: Arc<DoodadReference>,
        m2_resolver: Arc<Resolver<M2Generator, M2Node>>,
        tex_resolver: Arc<Resolver<M2Generator, RwLock<Option<IRTexture>>>>,
    ) {
        let handle_clone = handle.clone();
        set.spawn_on(
            async move {
                let reference_str = dad.reference.reference_str.clone();
                let result = handle_clone
                    .spawn_blocking(move || {
                        // TODO: PERF: get rid of duplicated references here, make DoodadReference#reference an Arc and cache them at least adt wide.
                        m2_resolver.resolve(reference_str)
                    })
                    .await
                    .unwrap();
                let mut new_set = JoinSet::new();
                // TODO: currently, we use join sets rarely, especially for those non-failing (error returning) operations, there's no reason to really join.
                Self::resolve_tex_reference(
                    &handle_clone,
                    &mut new_set,
                    tex_resolver,
                    result.tex_reference.clone(),
                );

                // TODO: With an async friendly RwLock, this could become regular async code, without spawn_blocking.
                handle_clone
                    .spawn_blocking(move || {
                        let mut write_lock = dad
                            .reference
                            .reference
                            .write()
                            .expect("Write lock on doodad reference");
                        *write_lock.deref_mut() = Some(result);
                    })
                    .await
                    .unwrap();

                while let Some(join) = new_set.join_next().await {
                    join.expect("Texture resolving to complete without panic");
                }
            },
            handle,
        );
    }

    fn resolve_tex_reference(
        handle: &Handle,
        set: &mut JoinSet<()>,
        tex_resolver: Arc<Resolver<M2Generator, RwLock<Option<IRTexture>>>>,
        references: Vec<Arc<IRTextureReference>>,
    ) {
        for tex_reference in references {
            let resolver = tex_resolver.clone();
            set.spawn_blocking_on(
                move || {
                    let result_tex = resolver.resolve(tex_reference.reference_str.clone());

                    let mut ref_wlock = tex_reference
                        .reference
                        .write()
                        .expect("texture reference write lock");

                    *ref_wlock.deref_mut() = Some(result_tex);
                },
                handle,
            );
        }
    }
}
