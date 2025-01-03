use std::net::TcpStream;
use std::ops::DerefMut;
use std::sync::mpsc::Sender;
use std::sync::{Mutex, OnceLock, RwLock, Weak};
use std::time::Instant;

use crate::game::application::GameApplication;
use crate::networking::movement_tracker::MovementTracker;
use crate::networking::skip_encrypted;
use itertools::Itertools;
use log::{info, warn};
use wow_srp::wrath_header::{ClientDecrypterHalf, ClientEncrypterHalf};
use wow_world_messages::Guid;
use wow_world_messages::errors::ExpectedOpcodeError;
use wow_world_messages::wrath::expect_server_message_encryption;
use wow_world_messages::wrath::opcodes::ServerOpcodeMessage;
use wow_world_messages::wrath::{
    CMSG_CHAR_ENUM, CMSG_PLAYER_LOGIN, SMSG_AUTH_RESPONSE, SMSG_AUTH_RESPONSE_WorldResult, SMSG_CHAR_ENUM,
};
use wow_world_messages::wrath::{
    CMSG_TIME_SYNC_RESP, ClientMessage, SMSG_CLIENTCACHE_VERSION, SMSG_TUTORIAL_FLAGS, SMSG_WARDEN_DATA,
};

pub struct WorldServer {
    stream: TcpStream,
    connect_time: Instant,
    packet_handler_sender: Sender<Box<ServerOpcodeMessage>>,

    /// The Encrypter for the packet cipher. Do not try to consecutively lock both encrypter and decrypter
    pub encrypter: Mutex<ClientEncrypterHalf>,
    /// The Decrypter for the packet cipher. Do not try to consecutively lock both encrypter and decrypter
    pub decrypter: Mutex<ClientDecrypterHalf>,

    pub player_guid: OnceLock<Guid>,
    pub movement_tracker: RwLock<MovementTracker>,
}

impl WorldServer {
    pub fn new(
        weak_self: Weak<WorldServer>,
        stream: TcpStream,
        encrypter: ClientEncrypterHalf,
        decrypter: ClientDecrypterHalf,
        packet_handler_sender: Sender<Box<ServerOpcodeMessage>>,
    ) -> Self {
        Self {
            stream,
            connect_time: Instant::now(),
            packet_handler_sender,
            encrypter: Mutex::new(encrypter),
            decrypter: Mutex::new(decrypter),
            movement_tracker: RwLock::new(MovementTracker::new(weak_self)),
            player_guid: OnceLock::new(),
        }
    }

    pub fn stream(&self) -> &TcpStream {
        &self.stream
    }

    pub fn run(&self, weak: Weak<GameApplication>) {
        {
            let mut dec = self.decrypter.lock().unwrap();
            let mut enc = self.encrypter.lock().unwrap();

            expect_server_message_encryption::<SMSG_WARDEN_DATA, _>(&mut self.stream(), dec.deref_mut()).unwrap();
            let s =
                expect_server_message_encryption::<SMSG_AUTH_RESPONSE, _>(&mut self.stream(), dec.deref_mut()).unwrap();

            if !matches!(s.result, SMSG_AUTH_RESPONSE_WorldResult::AuthOk { .. }) {
                panic!()
            }

            // we have to hack-skip this SMSG_ADDON_INFO packet as it can't be deserialized
            assert_eq!(
                0x02EFu16,
                skip_encrypted(self.stream(), dec.deref_mut()).unwrap()
            );
            expect_server_message_encryption::<SMSG_CLIENTCACHE_VERSION, _>(&mut self.stream(), dec.deref_mut())
                .unwrap();
            expect_server_message_encryption::<SMSG_TUTORIAL_FLAGS, _>(&mut self.stream(), dec.deref_mut()).unwrap();

            CMSG_CHAR_ENUM {}
                .write_encrypted_client(self.stream(), enc.deref_mut())
                .unwrap();

            let s = expect_server_message_encryption::<SMSG_CHAR_ENUM, _>(&mut self.stream(), dec.deref_mut()).unwrap();
            info!(
                "Characters: {:?}",
                &s.characters.iter().map(|char| &char.name).collect_vec()
            );

            if s.characters.is_empty() {
                panic!("This account doesn't have any characters yet, please create exactly one");
            }

            // TODO: Set that guid somewhere.
            let guid = s.characters[0].guid;
            self.player_guid.set(guid).expect("Setting possible");

            CMSG_PLAYER_LOGIN { guid }
                .write_encrypted_client(self.stream(), enc.deref_mut())
                .unwrap();
        }

        loop {
            if let Some(app) = weak.upgrade() {
                if app
                    .close_requested
                    .load(std::sync::atomic::Ordering::SeqCst)
                {
                    info!("App closing requested, shutting down");
                    return;
                }
            }

            let opcode_opt =
                ServerOpcodeMessage::read_encrypted(self.stream(), self.decrypter.lock().unwrap().deref_mut())
                    .map(Box::new);
            match opcode_opt {
                Err(err) => match err {
                    ExpectedOpcodeError::Opcode { .. } => warn!("Unimplemented opcode: {}", err),
                    ExpectedOpcodeError::Parse(error) => warn!("Parsing error: {}", error),
                    ExpectedOpcodeError::Io(error) => {
                        warn!("IO Error to server, connection broken: {}", error);
                        return;
                    }
                },
                Ok(opcode) => {
                    // TODO: comment back in, as soon as handle_packet doesn't also print unhandled opcode for nearly everything.
                    // trace!("SERVER: {}", opcode);
                    self.handle_packet(opcode);
                }
            }
        }
    }

    #[inline(always)]
    pub fn send_encrypted<M: ClientMessage>(&self, message: M) -> Result<(), std::io::Error> {
        log::trace!("Sending {}", message.message_name());
        message.write_encrypted_client(self.stream(), self.encrypter.lock().unwrap().deref_mut())
    }

    #[inline(always)]
    pub fn get_timestamp(&self) -> u32 {
        self.connect_time.elapsed().as_millis() as u32
    }

    fn handle_packet(&self, packet: Box<ServerOpcodeMessage>) {
        match packet.as_ref() {
            ServerOpcodeMessage::SMSG_TIME_SYNC_REQ(req) => {
                let time_sync = req.time_sync;
                let client_ticks = self.get_timestamp();
                self.send_encrypted(CMSG_TIME_SYNC_RESP {
                    time_sync,
                    client_ticks,
                })
                .unwrap();
            }
            _opcode => self
                .packet_handler_sender
                .send(packet)
                .expect("Failed to enqueue packet handlers"),
        }
    }
}
