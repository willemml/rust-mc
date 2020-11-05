use crate::{net::Packet, scanner::PacketScanner};

pub trait PacketHandler {
    fn handle_packet(&mut self, packet: Packet, scanner: &mut PacketScanner) where Self: Sized;
}
