use crate::entity::entity_tracker::EntityTracker;
use crate::entity::systems::systems::Systems;
use crate::game::game_state::GameState;
use crate::io::mpq::loader::MPQLoader;
use crate::networking::application::NetworkApplication;
use crate::physics::physics_state::PhysicsState;
use crate::rendering::application::RenderingApplication;
use crate::settings::CliArgs;
use chrono::{Local, Timelike};
use log::debug;
use rend3::Renderer;
use rend3::types::SampleCount;
use std::sync::atomic::AtomicBool;
use std::sync::mpsc::Receiver;
use std::sync::{Arc, OnceLock, Weak};
use winit::dpi::LogicalSize;
use wow_world_messages::wrath::opcodes::ServerOpcodeMessage;
use wow_world_messages::{DateTime, Month, Weekday};

pub enum GameOperationMode {
    Standalone,
    Networked(Receiver<Box<ServerOpcodeMessage>>),
}

pub struct GameApplication {
    pub mpq_loader: Arc<MPQLoader>,
    pub game_state: Arc<GameState>,
    pub close_requested: AtomicBool,
    pub renderer: OnceLock<Arc<Renderer>>,
    pub network: Option<NetworkApplication>,
    pub entity_tracker: EntityTracker,
    pub locale: String,
    systems: Systems,
    weak_self: Weak<GameApplication>,
}

const WINDOW_TITLE: &str = concat!(
    "Sargerust: Wrath of the Rust King (",
    env!("VERGEN_GIT_BRANCH"),
    "/",
    env!("VERGEN_GIT_SHA"),
    ")"
);

impl GameApplication {
    pub fn new(weak_self: &Weak<GameApplication>, mpq_loader: MPQLoader, args: &CliArgs) -> Self {
        let mpq_loader_arc = Arc::new(mpq_loader);
        Self {
            mpq_loader: mpq_loader_arc.clone(),
            weak_self: weak_self.clone(),
            game_state: Arc::new(GameState::new(weak_self.clone(), mpq_loader_arc.clone())),
            close_requested: AtomicBool::new(false),
            renderer: OnceLock::new(),
            entity_tracker: EntityTracker::new(),
            network: None,
            systems: Systems::new(weak_self.clone(), mpq_loader_arc.clone()),
            locale: args.locale.to_string(),
        }
    }

    pub fn connect_to_realm(
        &mut self,
        address: &str,
        username: &str,
        password: &str,
    ) -> Receiver<Box<ServerOpcodeMessage>> {
        let (network, receiver) = NetworkApplication::connect(address, username, password);
        self.network = Some(network);
        receiver
    }

    /// Run the game application. This will block until the window is closed and take care of
    /// starting and ending all the relevant threads. The Receiver is optional and only used when
    /// standalone == false and there has been a previous call to connect_to_realm.
    ///
    // TODO: Design flaw of the receiver. We can't hide it in the network application, though,
    //  it has to be consumed by spawning the network threads.
    pub fn run(&self, operation_mode: GameOperationMode) {
        let mut handles = match operation_mode {
            GameOperationMode::Networked(receiver) => self
                .network
                .as_ref()
                .expect("Network must be initialized in non-standalone mode")
                .spawn_networking_threads(self.weak_self.clone(), receiver),
            _ => vec![],
        };

        let wnd = winit::window::WindowAttributes::default()
            .with_title(WINDOW_TITLE)
            .with_inner_size(LogicalSize::new(1024, 768));
        let render_app = RenderingApplication::new(self.weak_self.clone(), SampleCount::Four);

        self.update_game_time();

        handles.push(PhysicsState::start(self.game_state.physics_state.clone()));

        rend3_framework::start(render_app, wnd); // This blocks until the window is closed

        debug!("Window closed, waiting for threads to finish");

        for handle in handles {
            handle.join().expect("Thread to terminate normally");
        }
    }

    fn update_game_time(&self) {
        let time = Local::now().time();
        let date_time = DateTime::new(
            0,
            Month::January,
            0,
            Weekday::Friday,
            time.hour() as u8,
            time.minute() as u8,
        );

        self.game_state
            .game_time
            .update_time_and_speed(date_time, 0.01666667);
    }

    pub fn logic_update(&self, delta_time: f32) {
        self.game_state.game_time.advance_time(delta_time);
        self.systems.update(self, delta_time);
    }
}
