use crate::handler::PacketHandler;
use crate::net::ServerConnection;
use crate::net::Packet;
use std::time::Duration;

pub struct PacketScanner<P: PacketHandler> {
    server: ServerConnection,
    running: bool,
    handler: P,
}

impl<P: PacketHandler> PacketScanner<P> {
    pub fn new(server: ServerConnection, handler: P) -> Self {
        let new_scanner = PacketScanner {
            server,
            running: false,
            handler,
        };
        return new_scanner;
    }

    pub async fn start(&mut self) {
        self.running = true;
        self.packet_scanner().await;
    }

    async fn packet_scanner(&mut self) {
        let sleep_time = Duration::from_millis(25);
        loop {
            if self.running {
                if let Ok(packet) = self.server.read_next_packet().await {
                    if let Some(packet) = packet {
                        self.handler.handle_packet(packet);
                    }
                }
            } else {
                break;
            }
            std::thread::sleep(sleep_time);
        }
    }

    pub fn stop(&mut self) {
        self.running = false;
    }
}

pub struct TestHandler;
impl PacketHandler for TestHandler {
    fn handle_packet(&mut self, _packet: Packet) {
        println!("Received a packet!")
    }
}
