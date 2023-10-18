use crate::game::application::GameApplication;
use crate::game::map_manager::MapManager;
use crate::io::mpq::loader::MPQLoader;
use glam::{Vec3, Vec3A};
use log::trace;
use std::sync::{Arc, RwLock, Weak};
use wow_world_messages::wrath::{Map, Vector3d};

/// This is _the_ shared state that is accessed by multiple threads
/// As always, ensure to NEVER acquire multiple mutexes at the same time
pub struct GameState {
    pub app: Weak<GameApplication>,
    pub map_manager: Arc<RwLock<MapManager>>,
    pub player_location: RwLock<Vec3A>,
    pub player_orientation: RwLock<f32>,
}

impl GameState {
    pub fn new(app: Weak<GameApplication>, mpq_loader: Arc<MPQLoader>) -> Self {
        Self {
            app,
            map_manager: Arc::new(RwLock::new(MapManager::new(mpq_loader))),
            player_location: RwLock::new(Vec3A::new(0.0, 0.0, 0.0)),
            player_orientation: RwLock::new(0.0),
        }
    }

    fn app(&self) -> Arc<GameApplication> {
        self.app.upgrade().expect("Weak Pointer expired")
    }

    /// Called when first entering the world and whenever the map changes (teleport, portal)
    pub fn change_map(&self, map: Map, position: Vector3d, orientation: f32) {
        trace!("Switching to map {}", map);

        // It's important to set the player location before loading the map for the first time,
        // because otherwise it could happen that we load the (32, 32) chunk (i.e. 0, 0, 0)
        let mut player_location = self
            .player_location
            .write()
            .expect("Player Location write lock");
        player_location.x = position.x;
        player_location.y = position.y;
        player_location.z = position.z;
        *self
            .player_orientation
            .write()
            .expect("Player Orientation write lock") = orientation;

        // TODO: temporary mapping from Eastern Kingdom to Azeroth.
        if map == Map::EasternKingdoms {
            self.map_manager.write().unwrap().preload_map(
                "Azeroth".into(),
                Vec3::new(position.x, position.y, position.z),
                orientation,
            );
        }
    }
}
