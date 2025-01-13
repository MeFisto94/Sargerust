use crate::entity::systems::display_id_resolver_system::DisplayIdResolverSystem;
use crate::entity::systems::rendering_system::RenderingSystem;
use crate::game::application::GameApplication;
use crate::io::mpq::loader::MPQLoader;
use std::sync::{Arc, Weak};

pub struct Systems {
    display_id_resolver_system: DisplayIdResolverSystem,
    rendering_system: RenderingSystem,
}

impl Systems {
    pub fn new(app: Weak<GameApplication>, mpq_loader: Arc<MPQLoader>) -> Self {
        Self {
            display_id_resolver_system: DisplayIdResolverSystem::new(mpq_loader),
            rendering_system: RenderingSystem::new(),
        }
    }

    pub fn update(&self, app: &GameApplication) {
        self.display_id_resolver_system.update(app);
        self.rendering_system.update(app);
    }
}
