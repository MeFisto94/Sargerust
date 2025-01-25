use crate::game::application::GameApplication;
use crate::game::map_manager::MapManager;
use crate::io::common::loader::RawAssetLoader;
use crate::io::mpq::loader::MPQLoader;
use crate::networking::utils::net_vector3d_to_glam;
use crate::physics::physics_state::PhysicsState;
use glam::Vec3A;
use log::{debug, error};
use std::io::Cursor;
use std::ops::Deref;
use std::sync::{Arc, RwLock, Weak};
use wow_dbc::DbcTable;
use wow_world_messages::wrath::{Map, Vector3d};

/// This is _the_ shared state that is accessed by multiple threads
/// As always, ensure to NEVER acquire multiple mutexes at the same time
pub struct GameState {
    pub app: Weak<GameApplication>,
    pub map_manager: Arc<RwLock<MapManager>>,
    // TODO: this is apparently in ADT space, this _has_ to be changed to blender space?
    pub player_location: RwLock<Vec3A>,
    pub player_orientation: RwLock<f32>,
    pub physics_state: Arc<RwLock<PhysicsState>>,
    map_dbc: wow_dbc::wrath_tables::map::Map,
}

impl GameState {
    pub fn new(app: Weak<GameApplication>, mpq_loader: Arc<MPQLoader>) -> Self {
        Self {
            map_manager: Arc::new(RwLock::new(MapManager::new(mpq_loader.clone()))),
            player_location: RwLock::new(Vec3A::new(0.0, 0.0, 0.0)),
            player_orientation: RwLock::new(0.0),
            physics_state: Arc::new(RwLock::new(PhysicsState::new(app.clone()))),
            app,
            map_dbc: Self::read_map(mpq_loader.deref()),
        }
    }

    fn app(&self) -> Arc<GameApplication> {
        self.app.upgrade().expect("Weak Pointer expired")
    }

    fn read_map(mpq_loader: &MPQLoader) -> wow_dbc::wrath_tables::map::Map {
        let map_buf = mpq_loader
            .load_raw_owned("DBFilesClient\\Map.dbc")
            .expect("Failed to load Map.dbc");

        wow_dbc::wrath_tables::map::Map::read(&mut Cursor::new(map_buf)).expect("Failed to parse Map.dbc")
    }

    pub fn change_map_from_string(&self, map_name: &str, position: Vector3d) {
        let map = Map::variants()
            .into_iter()
            .find(|m| format!("{}", m) == map_name);

        if let Some(map) = map {
            self.change_map(map, position, 0.0);
        } else {
            error!(
                "Could not find a map that matches the name {}. Whitespace differences?",
                map_name
            );
            panic!(
                "Could not find a map that matches the name {}. Whitespace differences?",
                map_name
            );
        }
    }

    /// Called when first entering the world and whenever the map changes (teleport, portal)
    pub fn change_map(&self, map: Map, position: Vector3d, orientation: f32) {
        let map_row = self
            .map_dbc
            .rows()
            .iter()
            .find(|row| row.id.id as u32 == map.as_int())
            .unwrap_or_else(|| panic!("Undefined Map {}", map));

        // TODO: Somehow handle locales
        debug!(
            "Switching to map {} (\"{}\", {})",
            map, map_row.map_name_lang.de_de, map_row.directory
        );

        // It's important to set the player location before loading the map for the first time,
        // because otherwise it could happen that we load the (32, 32) chunk (i.e. 0, 0, 0)
        let mut player_location = self
            .player_location
            .write()
            .expect("Player Location write lock");
        // TODO: adt_to_blender?
        player_location.x = position.x;
        player_location.y = position.y;
        player_location.z = position.z;
        *self
            .player_orientation
            .write()
            .expect("Player Orientation write lock") = orientation;

        self.map_manager.write().unwrap().preload_map(
            map_row.directory.clone(),
            net_vector3d_to_glam(position),
            orientation,
        );
    }
}
