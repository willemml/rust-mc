use super::super::Packet;
use crate::minecraft::client::Client;

pub trait PacketHandler {
    fn handle_packet(&self, packet: &Packet);
}

impl<'a> PacketHandler for Client<'a> {
    fn handle_packet(&self, packet: &Packet) {
        println!("yay it works!");
        match packet {
            Packet::LoginSuccess(body) => {
                println!("yay it works!")
            }
            _ => {}
        };
    }
}
