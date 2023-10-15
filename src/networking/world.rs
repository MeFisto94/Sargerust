use std::net::TcpStream;
use std::ops::DerefMut;
use std::sync::mpsc::Sender;
use std::sync::Mutex;
use std::time::SystemTime;

use itertools::Itertools;
use log::{info, trace, warn};
use wow_srp::wrath_header::{ClientDecrypterHalf, ClientEncrypterHalf};
use wow_world_messages::errors::ExpectedOpcodeError;
use wow_world_messages::wrath::expect_server_message_encryption;
use wow_world_messages::wrath::opcodes::ServerOpcodeMessage;
use wow_world_messages::wrath::{
    ClientMessage, CMSG_TIME_SYNC_RESP, SMSG_CLIENTCACHE_VERSION, SMSG_TUTORIAL_FLAGS, SMSG_WARDEN_DATA,
};
use wow_world_messages::wrath::{
    SMSG_AUTH_RESPONSE_WorldResult, CMSG_CHAR_ENUM, CMSG_PLAYER_LOGIN, SMSG_AUTH_RESPONSE, SMSG_CHAR_ENUM,
};

use crate::networking::skip_encrypted;

pub struct WorldServer {
    stream: TcpStream,
    connect_time: SystemTime,
    packet_handler_sender: Sender<Box<ServerOpcodeMessage>>,

    /// The Encrypter for the packet cipher. Do not try to consecutively lock both encrypter and decrypter
    pub encrypter: Mutex<ClientEncrypterHalf>,
    /// The Decrypter for the packet cipher. Do not try to consecutively lock both encrypter and decrypter
    pub decrypter: Mutex<ClientDecrypterHalf>,
}

impl WorldServer {
    pub fn new(
        stream: TcpStream,
        encrypter: ClientEncrypterHalf,
        decrypter: ClientDecrypterHalf,
        packet_handler_sender: Sender<Box<ServerOpcodeMessage>>,
    ) -> Self {
        Self {
            stream,
            connect_time: SystemTime::now(),
            packet_handler_sender,
            encrypter: Mutex::new(encrypter),
            decrypter: Mutex::new(decrypter),
        }
    }

    pub fn stream(&self) -> &TcpStream {
        &self.stream
    }

    pub fn run(&self) {
        // TODO: this does not support enabled warden, as warden is installed before the auth response apparently
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

            CMSG_PLAYER_LOGIN {
                guid: s.characters[0].guid,
            }
            .write_encrypted_client(self.stream(), enc.deref_mut())
            .unwrap();
        }

        loop {
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

    pub fn send_encrypted<M: ClientMessage>(&self, message: M) -> Result<(), std::io::Error> {
        message.write_encrypted_client(self.stream(), self.encrypter.lock().unwrap().deref_mut())
    }

    fn handle_packet(&self, packet: Box<ServerOpcodeMessage>) {
        match packet.as_ref() {
            ServerOpcodeMessage::SMSG_TIME_SYNC_REQ(req) => {
                let time_sync = req.time_sync;
                trace!("SYNC TIME: {}", &time_sync);
                let client_ticks = self.connect_time.elapsed().unwrap().as_millis() as u32;
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
