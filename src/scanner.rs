use crate::handler::PacketHandler;
use crate::net::ServerConnection;
use std::time::Duration;

pub struct PacketScanner {
    pub(crate) server: ServerConnection,
    pub(crate) running: bool,
}

impl PacketScanner {
    pub fn new(server: ServerConnection) -> Self {
        let new_scanner = PacketScanner {
            server,
            running: false,
        };
        return new_scanner;
    }

    pub async fn start<P>(&mut self, handler: &mut P) where P: PacketHandler {
        self.running = true;
        self.packet_scanner(handler).await;
    }

    async fn packet_scanner<P>(&mut self, handler: &mut P) where P: PacketHandler {
        let sleep_time = Duration::from_millis(25);
        loop {
            if self.running {
                if let Ok(packet) = self.server.read_next_packet().await {
                    if let Some(packet) = packet {
                        handler.handle_packet(packet, self);
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
