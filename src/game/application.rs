use rend3::Renderer;
use std::net::TcpStream;
use std::sync::atomic::AtomicBool;
use std::sync::mpsc::{Receiver, Sender, channel};
use std::sync::{Arc, OnceLock, Weak};
use std::thread::JoinHandle;

use winit::dpi::LogicalSize;
use wow_srp::normalized_string::NormalizedString;
use wow_srp::wrath_header::ProofSeed;
use wow_world_messages::wrath::opcodes::ServerOpcodeMessage;
use wow_world_messages::wrath::{
    CMSG_AUTH_SESSION, ClientMessage, Map, SMSG_AUTH_CHALLENGE, Vector3d, expect_server_message,
};

use crate::game::game_state::GameState;
use crate::game::packet_handlers::PacketHandlers;
use crate::io::mpq::loader::MPQLoader;
use crate::networking::application::NetworkApplication;
use crate::networking::auth;
use crate::networking::world::WorldServer;
use crate::rendering::application::RenderingApplication;

pub enum GameOperationMode {
    Standalone,
    Networked(Receiver<Box<ServerOpcodeMessage>>),
}

pub struct GameApplication {
    pub mpq_loader: Arc<MPQLoader>,
    // TODO: separation? one would expect the world server to carry all kinds of methods to emit and handle packets.
    // at the same time I want different structs for different threads, makes things easier.
    //pub packet_handlers: Option<Arc<PacketHandlers>>
    pub game_state: Arc<GameState>,
    pub close_requested: AtomicBool,
    pub renderer: OnceLock<Arc<Renderer>>,
    pub network: Option<NetworkApplication>,
    weak_self: Weak<GameApplication>,
}

impl GameApplication {
    pub fn new(weak_self: &Weak<GameApplication>, mpq_loader: MPQLoader) -> Self {
        let mpq_loader_arc = Arc::new(mpq_loader);
        Self {
            mpq_loader: mpq_loader_arc.clone(),
            weak_self: weak_self.clone(),
            game_state: Arc::new(GameState::new(weak_self.clone(), mpq_loader_arc.clone())),
            close_requested: AtomicBool::new(false),
            renderer: OnceLock::new(),
            network: None,
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
        let standalone = matches!(operation_mode, GameOperationMode::Standalone); // TODO: Sadly we have to move operation_mode's receiver. Better idea?

        let handles = match operation_mode {
            GameOperationMode::Networked(receiver) => self
                .network
                .as_ref()
                .expect("Network must be initialized in non-standalone mode")
                .spawn_networking_threads(self.weak_self.clone(), receiver),
            _ => vec![],
        };

        const WINDOW_TITLE: &str = concat!(
            "Sargerust: Wrath of the Rust King (",
            env!("VERGEN_GIT_BRANCH"),
            "/",
            env!("VERGEN_GIT_SHA"),
            ")"
        );

        let wnd = winit::window::WindowBuilder::new()
            .with_title(WINDOW_TITLE)
            .with_inner_size(LogicalSize::new(1024, 768));
        let render_app = RenderingApplication::new(self.weak_self.clone());

        if standalone {
            // TODO: Derive standalone *and* otherwise the map from the launch args.
            self.game_state.change_map(
                Map::EasternKingdoms,
                Vector3d {
                    x: -8924.0,
                    y: -117.0,
                    z: 82.0,
                },
                0.0,
            );
        }

        rend3_framework::start(render_app, wnd); // This blocks until the window is closed

        for handle in handles {
            handle
                .join()
                .expect("Networking thread to terminate normally");
        }
    }
}
