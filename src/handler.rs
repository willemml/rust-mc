use crate::net::Packet;

pub trait PacketHandler {
    fn handle_packet(&mut self, packet: Packet);
}
