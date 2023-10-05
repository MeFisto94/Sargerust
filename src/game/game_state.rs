use std::sync::{Arc, Weak};
use log::trace;
use wow_world_messages::wrath::{Map, Vector3d};
use crate::game::application::GameApplication;

/// This is _the_ shared state that is accessed by multiple threads
/// As always, ensure to NEVER acquire multiple mutexes at the same time
#[derive(Default)]
pub struct GameState {
    pub app: Weak<GameApplication>

}

impl GameState {
    pub fn new(app: Weak<GameApplication>) -> Self {
        Self {
            app,
            ..GameState::default()
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
            // Azeroth
        }

    }
}