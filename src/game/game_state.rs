use std::sync::{Arc, RwLock, Weak};
use glam::Vec3;
use log::trace;
use wow_world_messages::wrath::{Map, Vector3d};
use crate::game::application::GameApplication;
use crate::game::map_manager::MapManager;
use crate::io::mpq::loader::MPQLoader;

/// This is _the_ shared state that is accessed by multiple threads
/// As always, ensure to NEVER acquire multiple mutexes at the same time
pub struct GameState {
    pub app: Weak<GameApplication>,
    pub map_manager: Arc<RwLock<MapManager>>
}

impl GameState {
    pub fn new(app: Weak<GameApplication>, mpq_loader: Arc<MPQLoader>) -> Self {
        Self {
            app,
            map_manager: Arc::new(RwLock::new(MapManager::new(mpq_loader)))
        }
    }

    fn app(&self) -> Arc<GameApplication> {
        self.app.upgrade().expect("Weak Pointer expired")
    }

    /// Called when first entering the world and whenever the map changes (teleport, portal)
    pub fn change_map(&self, map: Map, position: Vector3d, orientation: f32) {
        trace!("Switching to map {}", map);
        // TODO: temporary mapping from Eastern Kingdom to Azeroth.
        if map == Map::EasternKingdoms {
            self.map_manager.write().unwrap().preload_map("Azeroth".into(), Vec3::new(position.x, position.y, position.z), orientation);
        }

    }
}