use super::super::Packet;
use crate::minecraft::client::Client;

pub trait PacketHandler {
    fn handle_packet(&self, packet: &Packet);
}

impl<'a> PacketHandler for Client {
    fn handle_packet(&self, packet: &Packet) {
        match packet {
            Packet::PlayServerChatMessage(body) => {
                if body.sender != self.profile.game_profile.id {
                    if let Some(message) = body.message.to_traditional() {
                        println!("{}", message);
                    } else {
                        println!("Raw message: {:?}", body);
                    }
                }
            }
            _ => {}
        };
    }
}
