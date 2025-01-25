use crate::game::application::GameApplication;
use crate::game::packet_handlers::PacketHandlers;
use crate::networking::auth;
use crate::networking::world::WorldServer;
use log::trace;
use std::net::TcpStream;
use std::sync::mpsc::{Receiver, Sender, channel};
use std::sync::{Arc, Weak};
use std::thread::JoinHandle;
use wow_login_messages::version_8::Realm;
use wow_srp::SESSION_KEY_LENGTH;
use wow_srp::normalized_string::NormalizedString;
use wow_srp::wrath_header::ProofSeed;
use wow_world_messages::wrath::opcodes::ServerOpcodeMessage;
use wow_world_messages::wrath::{CMSG_AUTH_SESSION, ClientMessage, SMSG_AUTH_CHALLENGE, expect_server_message};

pub struct NetworkApplication {
    pub world_server: Arc<WorldServer>,
}

impl NetworkApplication {
    pub fn connect(
        address: &str,
        username: &str,
        password: &str,
    ) -> (NetworkApplication, Receiver<Box<ServerOpcodeMessage>>) {
        let (session_key, realms) = Self::logon_realm(address, username, password);
        trace!("Choosing realm {}", &realms[0].name);

        let (sender, receiver) = channel();

        (
            Self {
                world_server: NetworkApplication::connect_to_world_server(sender, username, &realms[0], session_key),
            },
            receiver,
        )
    }

    fn logon_realm(address: &str, username: &str, password: &str) -> ([u8; SESSION_KEY_LENGTH as usize], Vec<Realm>) {
        trace!(
            "Connecting to the auth server at {} for user {}",
            address, username
        );
        let mut auth_server = TcpStream::connect(address).expect("Connecting to the Server succeeds");
        let (key, realm_msg) = auth::auth(&mut auth_server, username, password);
        (key, realm_msg.realms)
    }

    fn connect_to_world_server(
        packet_handler_sender: Sender<Box<ServerOpcodeMessage>>,
        username: &str,
        realm: &Realm,
        session_key: [u8; SESSION_KEY_LENGTH as usize],
    ) -> Arc<WorldServer> {
        let server_id = realm.realm_id; // TODO: inline
        let world_server_stream = TcpStream::connect(&realm.address).unwrap();

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

        Arc::new_cyclic(|weak| {
            WorldServer::new(
                weak.clone(),
                world_server_stream,
                encrypter,
                decrypter,
                packet_handler_sender,
            )
        })
    }

    fn spawn_packet_handler_thread(
        &self,
        game: Weak<GameApplication>,
        receiver: Receiver<Box<ServerOpcodeMessage>>,
    ) -> JoinHandle<()> {
        std::thread::Builder::new()
            .name("Packet Handlers".into())
            .spawn(|| PacketHandlers::new(game, receiver).run())
            .expect("Spawning the Packet Handlers Thread succeeds")
    }

    fn spawn_world_server_thread(&self, game: Weak<GameApplication>) -> JoinHandle<()> {
        WorldServer::spawn_thread(self.world_server.clone(), game)
    }

    pub fn spawn_networking_threads(
        &self,
        game: Weak<GameApplication>,
        receiver: Receiver<Box<ServerOpcodeMessage>>,
    ) -> Vec<JoinHandle<()>> {
        let game_server = self.spawn_world_server_thread(game.clone());
        let packet = self.spawn_packet_handler_thread(game, receiver);

        vec![game_server, packet]
    }
}
