use crate::Client;

use super::super::net::handler::PacketHandler;
use mpsc::Receiver;
use std::{sync::Arc, time::Duration};
use tokio::sync::{
    mpsc::{self, error::TryRecvError},
    Mutex,
};

pub struct PacketScanner;

impl PacketScanner {
    pub async fn start(client: Arc<Mutex<Client>>, rx: Arc<Mutex<Receiver<()>>>) {
        loop {
            if let Err(err) = rx.lock().await.try_recv() {
                if err == TryRecvError::Closed {
                    break;
                }
                let client_lock = &mut client.lock().await;
                if let Some(server) = &mut client_lock.server {
                    if let Ok(packet) = &mut server.read_next_packet().await {
                        if let Some(packet) = packet {
                            client_lock.handle_packet(packet);
                        }
                    }
                } else {
                    break;
                };
            }
        }
    }
}
