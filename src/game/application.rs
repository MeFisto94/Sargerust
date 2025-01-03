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
use wow_world_messages::wrath::{CMSG_AUTH_SESSION, ClientMessage, SMSG_AUTH_CHALLENGE, expect_server_message};

use crate::game::game_state::GameState;
use crate::game::packet_handlers::PacketHandlers;
use crate::io::mpq::loader::MPQLoader;
use crate::networking::auth;
use crate::networking::world::WorldServer;
use crate::rendering::application::RenderingApplication;

pub struct GameApplication {
    pub mpq_loader: Arc<MPQLoader>,
    pub world_server: Option<Arc<WorldServer>>,
    // TODO: separation? one would expect the world server to carry all kinds of methods to emit and handle packets.
    // at the same time I want different structs for different threads, makes things easier.
    //pub packet_handlers: Option<Arc<PacketHandlers>>
    pub game_state: Arc<GameState>,
    pub close_requested: AtomicBool,
    pub renderer: OnceLock<Arc<Renderer>>,
    weak_self: Weak<GameApplication>,
}

impl GameApplication {
    pub fn new(weak_self: &Weak<GameApplication>, mpq_loader: MPQLoader) -> Self {
        let mpq_loader_arc = Arc::new(mpq_loader);
        Self {
            mpq_loader: mpq_loader_arc.clone(),
            weak_self: weak_self.clone(),
            world_server: None,
            game_state: Arc::new(GameState::new(weak_self.clone(), mpq_loader_arc.clone())),
            close_requested: AtomicBool::new(false),
            renderer: OnceLock::new(),
        }
    }

    pub fn realm_logon(&mut self, address: &str, username: &str, password: &str) -> Receiver<Box<ServerOpcodeMessage>> {
        let (sender, receiver) = channel();
        self.prepare_network(sender, address, username, password);
        receiver
    }

    pub fn run(&self, receiver: Receiver<Box<ServerOpcodeMessage>>) {
        let ws = self.world_server.as_ref().unwrap().clone();
        let cloned_self = self.weak_self.clone();
        let net_thread = std::thread::Builder::new()
            .name("Network".into())
            .spawn(move || {
                ws.run(cloned_self);
            })
            .unwrap();

        let logic_thread = self.run_packet_handlers(receiver);

        let wnd = winit::window::WindowBuilder::new()
            .with_title("Sargerust: Wrath of the Rust King")
            .with_inner_size(LogicalSize::new(1024, 768));
        let render_app = RenderingApplication::new(self.weak_self.clone());
        rend3_framework::start(render_app, wnd);

        net_thread
            .join()
            .expect("Network Thread to terminate normally");
        logic_thread
            .join()
            .expect("Logic Thread to terminate normally");
    }

    fn prepare_network(
        &mut self,
        packet_handler_sender: Sender<Box<ServerOpcodeMessage>>,
        address: &str,
        username: &str,
        password: &str,
    ) {
        let (session_key, world_server_stream, server_id) = {
            let mut auth_server = TcpStream::connect(address).expect("Connecting to the Server succeeds");
            let (session_key, realms) = auth::auth(&mut auth_server, username, password);
            log::trace!("Choosing realm {}", &realms.realms[0].name);
            (
                session_key,
                TcpStream::connect(&realms.realms[0].address).unwrap(),
                realms.realms[0].realm_id,
            )
        };

        // Got the realm, have been connecting to the world server
        let s = expect_server_message::<SMSG_AUTH_CHALLENGE, _>(&mut &world_server_stream).unwrap();

        let seed = ProofSeed::new();
        let seed_value = seed.seed();
        let (client_proof, crypto) = seed.into_client_header_crypto(
            &NormalizedString::new(username).unwrap(),
            session_key,
            s.server_seed,
        );

        // Caution, crypto implements Copy and then encryption breaks! Do not access encrypt/decrypt here, use the world server.
        let (encrypter, decrypter) = crypto.split();

        // AddonInfo {
        //     addon_name: "Test".to_string(),
        //     addon_crc: 0,
        //     addon_extra_crc: 0,
        //     addon_has_signature: 0,
        // }

        CMSG_AUTH_SESSION {
            client_build: 12340,
            login_server_id: server_id as u32,
            // The trick is that we need to uppercase the account name
            username: NormalizedString::new(username).unwrap().to_string(),
            client_seed: seed_value,
            client_proof,
            addon_info: vec![],
            login_server_type: 0, // 0 == "grunt" and 1 == "battle net"??
            region_id: 0,
            battleground_id: 0,
            realm_id: server_id as u32,
            dos_response: 0,
        }
        .write_unencrypted_client(&mut &world_server_stream)
        .unwrap();

        let ws_arc = Arc::new_cyclic(|weak| {
            WorldServer::new(
                weak.clone(),
                world_server_stream,
                encrypter,
                decrypter,
                packet_handler_sender,
            )
        });
        self.world_server = Some(ws_arc);
    }

    pub fn run_packet_handlers(&self, receiver: Receiver<Box<ServerOpcodeMessage>>) -> JoinHandle<()> {
        let weak = self.weak_self.clone();
        std::thread::Builder::new()
            .name("Packet Handlers".into())
            .spawn(|| {
                PacketHandlers::new(weak, receiver).run();
            })
            .unwrap()
    }
}
