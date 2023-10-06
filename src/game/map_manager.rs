use std::collections::HashMap;
use std::io::Cursor;
use std::ops::Add;
use std::rc::Rc;
use std::sync::Arc;
use glam::{Affine3A, Vec3};
use image_blp::BlpImage;
use log::{error, info, trace, warn};
use sargerust_files::adt::reader::ADTReader;
use sargerust_files::adt::types::ADTAsset;
use sargerust_files::wdt::reader::WDTReader;
use sargerust_files::wdt::types::WDTAsset;
use crate::io::common::loader::RawAssetLoader;
use crate::io::mpq::loader::MPQLoader;
use crate::{handle_adt, rendering};
use crate::rendering::common::coordinate_systems;
use crate::rendering::common::types::{Material, Mesh};

pub struct MapManager {
    mpq_loader: Arc<MPQLoader>,
    current_map: Option<(String, WDTAsset)>,
    loaded_tiles: HashMap<(u8, u8), Box<ADTAsset>>
}

impl MapManager {
    pub fn new(mpq_loader: Arc<MPQLoader>) -> Self {
        Self {
            mpq_loader,
            current_map: None,
            loaded_tiles: HashMap::new()
        }
    }

    pub fn preload_map(&mut self, map: String, position: Vec3, orientation: f32) {
        info!("Loading map {} @ {}", map, position);
        let wdt_buf = self.mpq_loader.as_ref().load_raw_owned(&format!("world\\maps\\{}\\{}.wdt", map, map));
        let wdt = WDTReader::parse_asset(&mut Cursor::new(wdt_buf.expect("Cannot load map wdt"))).expect("Error parsing WDT");

        let chunk_coords = coordinate_systems::adt_world_to_tiles(position);
        // TODO: We expect the result to be (row, column), but for some reason, it seems to be (column, row)

        if wdt.has_chunk(chunk_coords.1, chunk_coords.0) {
            let adt_buf = self.mpq_loader.as_ref().load_raw_owned(&format!("world\\maps\\{}\\{}_{}_{}.adt", map, map, chunk_coords.1, chunk_coords.0));
            let adt = ADTReader::parse_asset(&mut Cursor::new(adt_buf.expect("Cannot load map adt"))).expect("Error parsing ADT");
            trace!("Loaded tile {}_{}_{}", map, chunk_coords.1, chunk_coords.0);

            let mut m2_cache: HashMap<String, Rc<(Mesh, Material, Option<BlpImage>)>> = HashMap::new();
            let mut render_list: Vec<(Affine3A, Rc<(Mesh, Material, Option<BlpImage>)>)> = Vec::new();
            let mut texture_map = HashMap::new();
            let mut wmos = Vec::new();

            let terrain_chunk = handle_adt(&self.mpq_loader, &adt, &mut m2_cache, &mut render_list, &mut texture_map, &mut wmos).unwrap();
            self.loaded_tiles.insert(chunk_coords, adt);
            rendering::render(render_list, wmos.iter().map(|wmo| (&wmo.0, &wmo.1)), texture_map, terrain_chunk, coordinate_systems::adt_to_blender(position.into()));
        } else {
            error!("We load into the world on unmapped terrain?!");
        }

        self.current_map = Some((map, wdt));

        // ADT file is map_x_y.adt. I think x are rows and ys are columns.
    }
}