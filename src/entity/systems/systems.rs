use crate::audio::zone_music_manager::ZoneMusicManager;
use crate::entity::systems::display_id_resolver_system::DisplayIdResolverSystem;
use crate::entity::systems::rendering_system::RenderingSystem;
use crate::entity::systems::spline_walker_system::SplineWalkerSystem;
use crate::game::application::GameApplication;
use crate::io::mpq::loader::MPQLoader;
use crate::settings::CliArgs;
use log::debug;
use std::sync::{Arc, RwLock};
use std::time::Instant;

pub struct Systems {
    display_id_resolver_system: DisplayIdResolverSystem,
    rendering_system: RenderingSystem,
    spline_walker_system: SplineWalkerSystem,
    zone_music_manager: RwLock<ZoneMusicManager>,
}

impl Systems {
    pub fn new(mpq_loader: Arc<MPQLoader>, args: &CliArgs) -> Self {
        Self {
            display_id_resolver_system: DisplayIdResolverSystem::new(mpq_loader.clone()),
            rendering_system: RenderingSystem::new(),
            spline_walker_system: SplineWalkerSystem::new(),
            zone_music_manager: RwLock::new(ZoneMusicManager::new(mpq_loader, args)),
        }
    }

    pub fn update(&self, app: &GameApplication, delta_time: f32) {
        let pre_systems = Instant::now();

        // Technically, since all of those systems are internal and sequential, we could rather RwLock
        // Systems, but if we want to execute those systems in parallel in the future, individual locks
        // helps.
        self.spline_walker_system.update(app, delta_time);
        self.display_id_resolver_system.update(app);
        self.rendering_system.update(app);
        self.zone_music_manager
            .write()
            .expect("lock poisoned")
            .update(app, delta_time);

        let duration_systems = (Instant::now() - pre_systems).as_millis();
        if duration_systems > 6 {
            debug!("Systems update took too long: {:?} ms", duration_systems);
        }
    }
}
