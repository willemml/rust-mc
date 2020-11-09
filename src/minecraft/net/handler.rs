use super::super::Packet;
use crate::minecraft::client::Client;

/// A packet handler, handles packets for the Minecraft client.
/// Currently a trait to keep it in a different file from the client to avoid over cluttering and to improve readability.
pub trait PacketHandler {
    /// Handles packets.
    fn handle_packet(&self, packet: &Packet);
}

/// Handles client bound packets.
impl PacketHandler for Client {
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
