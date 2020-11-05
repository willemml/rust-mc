use crate::handler::PacketHandler;
use crate::net::ServerConnection;
use std::sync::{Arc, Mutex};
use std::time::Duration;

pub struct PacketScanner<P: PacketHandler>
{
    server: ServerConnection,
    running: bool,
    handler: P,
}

impl<P: PacketHandler + Sync + Send> PacketScanner<P>
{
    fn new(server: ServerConnection, handler: P) -> Self {
        let new_scanner = PacketScanner {
            server,
            running: false,
            handler,
        };
        return new_scanner;
    }

    fn start(&'static mut self) {
        let self_mutex = Arc::new(Mutex::new(self));
        //tokio::task::spawn(Self::packet_scanner(self_mutex));
    }

    async fn packet_scanner(self_mutex: Arc<Mutex<&mut Self>>) {
        let sleep_time = Duration::from_millis(25);
        loop {
            if let Ok(scanner) = self_mutex.lock().as_mut() {
                if scanner.running {
                    if let Ok(packet) = scanner.server.read_next_packet().await {
                        if let Some(packet) = packet {
                            scanner.handler.handle_packet(packet);
                        }
                    }
                } else {
                    break;
                }
            } else {
                break;
            };
            std::thread::sleep(sleep_time);
        }
    }

    fn stop(&mut self) {
        self.running = false;
    }
}
