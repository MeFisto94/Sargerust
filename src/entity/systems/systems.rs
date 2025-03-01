use crate::entity::systems::display_id_resolver_system::DisplayIdResolverSystem;
use crate::entity::systems::rendering_system::RenderingSystem;
use crate::entity::systems::spline_walker_system::SplineWalkerSystem;
use crate::game::application::GameApplication;
use crate::io::mpq::loader::MPQLoader;
use log::debug;
use std::sync::{Arc, Weak};
use std::time::Instant;

pub struct Systems {
    display_id_resolver_system: DisplayIdResolverSystem,
    rendering_system: RenderingSystem,
    spline_walker_system: SplineWalkerSystem,
}

impl Systems {
    pub fn new(_app: Weak<GameApplication>, mpq_loader: Arc<MPQLoader>) -> Self {
        Self {
            display_id_resolver_system: DisplayIdResolverSystem::new(mpq_loader),
            rendering_system: RenderingSystem::new(),
            spline_walker_system: SplineWalkerSystem::new(),
        }
    }

    pub fn update(&self, app: &GameApplication, delta_time: f32) {
        let pre_systems = Instant::now();

        self.spline_walker_system.update(app, delta_time);
        self.display_id_resolver_system.update(app);
        self.rendering_system.update(app);

        let duration_systems = (Instant::now() - pre_systems).as_millis();
        if duration_systems > 6 {
            debug!("Systems update took too long: {:?} ms", duration_systems);
        }
    }
}
