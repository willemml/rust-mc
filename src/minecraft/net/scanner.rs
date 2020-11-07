use crate::Client;

use super::super::net::handler::PacketHandler;
use mpsc::{Receiver, Sender};
use std::{sync::Arc, time::Duration};
use tokio::sync::{
    mpsc::{self, error::TryRecvError, error::TrySendError},
    Mutex,
};

pub struct PacketScanner<'a> {
    pub(crate) client: Arc<Mutex<&'a mut Client<'a>>>,
    tx: Sender<()>,
    rx: Receiver<()>,
}

impl<'a> PacketScanner<'a> {
    pub fn new(client: &'a mut Client<'a>) -> Self {
        let (tx, rx) = mpsc::channel(20);
        let new_scanner = PacketScanner {
            client: Arc::new(Mutex::new(client)),
            tx,
            rx,
        };
        return new_scanner;
    }

    pub fn start(self) -> Stopper {
        let stopper = Stopper { tx: self.tx };
        let scanner_info = (self.client, self.rx);
        //std::thread::spawn(move || Self::start_thread(scanner_info.0, scanner_info.1));
        stopper
    }

    async fn start_thread(client: Arc<Mutex<&'a mut Client<'a>>>, rx: Receiver<()>) {
        Self::packet_scanner(25, rx, client).await;
    }

    async fn packet_scanner(
        interval: u64,
        mut rx: Receiver<()>,
        client: Arc<Mutex<&'a mut Client<'a>>>,
    ) {
        let sleep_time = Duration::from_millis(interval);
        loop {
            if let Err(err) = rx.try_recv() {
                if err == TryRecvError::Closed {
                    break;
                }
                if let Ok(client) = &mut client.try_lock().as_deref_mut() {
                    if let Some(server) = &mut client.server {
                        if let Ok(packet) = &mut server.read_next_packet().await {
                            if let Some(packet) = packet {
                                client.handle_packet(packet);
                            }
                        }
                    } else {
                        break;
                    }
                }
                std::thread::sleep(sleep_time);
            } else {
                break;
            };
        }
    }
}

pub struct Stopper {
    tx: Sender<()>
}

impl Stopper {
    pub fn stop(mut self) -> std::result::Result<(), TrySendError<()>> {
        self.tx.try_send(())
    }
}
