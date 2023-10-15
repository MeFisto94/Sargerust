use std::sync::mpsc::Receiver;
use std::sync::{Arc, Weak};

use log::{info, warn};
use wow_world_messages::wrath::opcodes::ServerOpcodeMessage;

use crate::game::application::GameApplication;

pub struct PacketHandlers {
    app: Weak<GameApplication>,
    receiver: Receiver<Box<ServerOpcodeMessage>>,
}

impl PacketHandlers {
    pub fn new(app: Weak<GameApplication>, receiver: Receiver<Box<ServerOpcodeMessage>>) -> Self {
        Self { app, receiver }
    }

    fn app(&self) -> Arc<GameApplication> {
        self.app.upgrade().expect("Weak Pointer expired")
    }

    pub fn run(&self) {
        loop {
            let res = self.receiver.recv();
            if res.is_err() {
                warn!("PacketHandlers: Broken Pipe");
                return;
            }

            match res.unwrap().as_ref() {
                ServerOpcodeMessage::SMSG_LOGIN_VERIFY_WORLD(pkt) => {
                    // pkt.as_int() and then manual DBC logic at some point, to support custom maps.

                    self.app()
                        .game_state
                        .change_map(pkt.map, pkt.position, pkt.orientation);
                    // here, we would probably want to call into the GameApplication again.
                }
                ServerOpcodeMessage::SMSG_MONSTER_MOVE(_) => (),
                ServerOpcodeMessage::SMSG_MOTD(pkt) => {
                    for motd in &pkt.motd {
                        info!("MOTD: {}", motd)
                    }
                }
                ServerOpcodeMessage::SMSG_MESSAGECHAT(chat) => info!("CHAT: {}", &chat.message),
                opcode => info!("Unhandled opcode: {}", opcode),
            }
        }
    }
}
