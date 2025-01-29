use log::{info, warn};
use std::sync::atomic::Ordering::SeqCst;
use std::sync::mpsc::Receiver;
use std::sync::{Arc, Weak};
use std::time::Duration;
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
            if self.app().close_requested.load(SeqCst) {
                info!("App closing requested, shutting down");
                return;
            }

            let res = self.receiver.recv_timeout(Duration::from_millis(100));

            if let Err(std::sync::mpsc::RecvTimeoutError::Timeout) = res {
                continue;
            } else if res.is_err() {
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
                    for motd in &pkt.motds {
                        info!("MOTD: {}", motd)
                    }
                }
                ServerOpcodeMessage::SMSG_MESSAGECHAT(chat) => info!("CHAT: {}", &chat.message),
                ServerOpcodeMessage::SMSG_COMPRESSED_UPDATE_OBJECT(obj) => {
                    self.app().entity_tracker.update_objects(&obj.objects);
                }
                ServerOpcodeMessage::SMSG_UPDATE_OBJECT(obj) => {
                    self.app().entity_tracker.update_objects(&obj.objects);
                }
                ServerOpcodeMessage::SMSG_DESTROY_OBJECT(obj) => {
                    self.app()
                        .entity_tracker
                        .destroy_object(obj.guid, obj.target_died);
                }
                ServerOpcodeMessage::SMSG_LOGIN_SETTIMESPEED(obj) => {
                    let time = obj.datetime;
                    let scale = obj.timescale;

                    self.app()
                        .game_state
                        .game_time
                        .update_time_and_speed(time, scale);
                }
                opcode => info!("Unhandled opcode: {}", opcode),
            }
        }
    }
}
