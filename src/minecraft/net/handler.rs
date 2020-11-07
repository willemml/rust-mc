use super::super::Packet;
use crate::minecraft::client::Client;

pub trait PacketHandler {
    fn handle_packet(&self, packet: &Packet);
}

impl<'a> PacketHandler for Client {
    fn handle_packet(&self, packet: &Packet) {
        match packet {
            Packet::PlayServerChatMessage(body) => {
                if let Some(message) = body.message.to_traditional() {
                    println!("Received message: {}", message);
                } else {
                    println!("Raw message: {:?}", body);
                }
            }
            _ => {}
        };
    }
}
