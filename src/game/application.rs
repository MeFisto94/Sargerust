use std::net::TcpStream;
use std::sync::{Arc, Weak};
use std::sync::mpsc::{channel, Receiver, Sender};
use std::thread::JoinHandle;
use glam::UVec2;
use rend3::Renderer;
use rend3::types::{Handedness, SampleCount, Surface, TextureFormat};
use rend3_framework::{DefaultRoutines, Event, UserResizeEvent};
use rend3_routine::base::BaseRenderGraph;
use winit::dpi::LogicalSize;
use winit::event_loop::{ControlFlow, EventLoop};
use winit::window::Window;
use wow_srp::normalized_string::NormalizedString;
use wow_srp::wrath_header::ProofSeed;
use wow_world_messages::wrath::{ClientMessage, CMSG_AUTH_SESSION, expect_server_message, SMSG_AUTH_CHALLENGE};
use wow_world_messages::wrath::opcodes::ServerOpcodeMessage;
use crate::game::game_state::GameState;
use crate::game::packet_handlers::PacketHandlers;
use crate::networking::{auth};
use crate::networking::world::WorldServer;

#[derive(Default)]
pub struct GameApplication {
    pub world_server: Option<Arc<WorldServer>>,
    // TODO: separation? one would expect the world server to carry all kinds of methods to emit and handle packets.
    // at the same time I want different structs for different threads, makes things easier.
    //pub packet_handlers: Option<Arc<PacketHandlers>>

    pub game_state: Arc<GameState>,
    weak_self: Weak<GameApplication>
}

impl GameApplication {
    pub fn new(weak_self: &Weak<GameApplication>) -> Self {
        GameApplication { world_server: None, game_state: Arc::new(GameState::new(weak_self.clone())), weak_self: weak_self.clone() }
    }

    pub fn realm_logon(&mut self) -> Receiver<Box<ServerOpcodeMessage>> {
        let (sender, receiver) = channel();
        self.prepare_network(sender);
        receiver
    }

    pub fn run(&self, receiver: Receiver<Box<ServerOpcodeMessage>>) {
        let ws = self.world_server.as_ref().unwrap().clone();
        let net_thread = std::thread::Builder::new().name("Network".into())
            .stack_size(8000000).spawn(move || { ws.run(); }).unwrap();

        let logic_thread = self.run_packet_handlers(receiver);

        net_thread.join().expect("Network Thread to terminate normally");
        logic_thread.join().expect("Logic Thread to terminate normally");
        return;
        /*rend3_framework::start(self,
            winit::window::WindowBuilder::new()
                .with_title("Sargerust: Wrath of the Rust King")
                .with_inner_size(LogicalSize::new(1024, 768))
        );*/
    }

    fn prepare_network(&mut self, packet_handler_sender: Sender<Box<ServerOpcodeMessage>>) {
        let (session_key, world_server_stream, server_id) = {
            let mut auth_server = TcpStream::connect("192.168.0.196:3724").unwrap();
            let (session_key, realms) = auth::auth(&mut auth_server, "admin", "admin");
            dbg!(&realms.realms[0].name);
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
            &NormalizedString::new("admin").unwrap(), session_key, s.server_seed);

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
            username: NormalizedString::new("admin").unwrap().to_string(),
            client_seed: seed_value,
            client_proof,
            addon_info: vec![],
            login_server_type: 0, // 0 == "grunt" and 1 == "battle net"??
            region_id: 0,
            battleground_id: 0,
            realm_id: server_id as u32,
            dos_response: 0,
        }.write_unencrypted_client(&mut &world_server_stream).unwrap();

        self.world_server = Some(Arc::new(WorldServer::new(world_server_stream, encrypter, decrypter, packet_handler_sender)));
    }

    pub fn run_packet_handlers(&self, receiver: Receiver<Box<ServerOpcodeMessage>>) -> JoinHandle<()> {
        let weak = self.weak_self.clone();
        std::thread::Builder::new().name("Packet Handlers".into()).spawn(|| {
            PacketHandlers::new(weak, receiver).run();
        }).unwrap()
    }
}

impl rend3_framework::App for GameApplication {
    const HANDEDNESS: Handedness = Handedness::Right;

    fn register_logger(&mut self) {
        // intentionally no-opped.
    }

    fn sample_count(&self) -> SampleCount {
        SampleCount::One // No MSAA yet
    }

    fn setup(&mut self, event_loop: &EventLoop<UserResizeEvent<()>>, window: &Window, renderer: &Arc<Renderer>, routines: &Arc<DefaultRoutines>, surface_format: TextureFormat) {
        // TODO: stuff.
    }

    fn handle_event(&mut self, window: &Window, renderer: &Arc<Renderer>, routines: &Arc<DefaultRoutines>, base_rendergraph: &BaseRenderGraph, surface: Option<&Arc<Surface>>, resolution: UVec2, event: Event<'_, ()>, control_flow: impl FnOnce(ControlFlow)) {
        match event {
            // Close button was clicked, we should close.
            rend3_framework::Event::WindowEvent {
                event: winit::event::WindowEvent::CloseRequested,
                ..
            } => {
                control_flow(winit::event_loop::ControlFlow::Exit);
            }
            rend3_framework::Event::MainEventsCleared => {
                window.request_redraw();
            }
            // Render!
            rend3_framework::Event::RedrawRequested(_) => {
                // Get a frame
                let frame = surface.unwrap().get_current_texture().unwrap();

                // Swap the instruction buffers so that our frame's changes can be processed.
                renderer.swap_instruction_buffers();
                // Evaluate our frame's world-change instructions
                let mut eval_output = renderer.evaluate_instructions();

                // Lock the routines
                let pbr_routine = rend3_framework::lock(&routines.pbr);
                let tonemapping_routine = rend3_framework::lock(&routines.tonemapping);

                // Build a rendergraph
                let mut graph = rend3::graph::RenderGraph::new();

                // Import the surface texture into the render graph.
                let frame_handle =
                    graph.add_imported_render_target(&frame, 0..1, rend3::graph::ViewportRect::from_size(resolution));
                // Add the default rendergraph without a skybox
                base_rendergraph.add_to_graph(
                    &mut graph,
                    &eval_output,
                    &pbr_routine,
                    None,
                    &tonemapping_routine,
                    frame_handle,
                    resolution,
                    self.sample_count(),
                    glam::Vec4::ZERO,
                    glam::Vec4::new(0.10, 0.05, 0.10, 1.0), // Nice scene-referred purple
                );

                // Dispatch a render using the built up rendergraph!
                graph.execute(renderer, &mut eval_output);

                // Present the frame
                frame.present();
            }
            // Other events we don't care about
            _ => {}
        }
    }
}