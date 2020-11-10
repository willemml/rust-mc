use std::sync::Arc;

use tokio::{net::TcpListener, sync::Mutex};

use super::net::connection::MinecraftConnection;

pub struct Server {
    connections: Arc<Mutex<Vec<MinecraftConnection>>>,
    bind_address: String,
}

impl Server {
    pub fn new(bind_address: String) -> Self {
        Server {
            connections: Arc::new(Mutex::new(vec![])),
            bind_address,
        }
    }

    pub async fn start(&mut self) {
        let connections = self.connections.clone();
        let address = self.bind_address.clone();
        let mut listener = TcpListener::bind(address).await;
        if let Ok(listener) = &mut listener {
            loop {
                if let Ok((socket, address)) = listener.accept().await {
                    let mut client = MinecraftConnection::from_tcp_stream(socket);
                    if let Err(error) = client.handshake(None, None, None, Some(256)).await {
                        println!("Handshake with {} failed: {}", address.to_string(), error)
                    } else {
                        println!("Handshake with {} successful.", address.to_string())
                    }
                    connections.lock().await.push(client);
                }
            }
        }
    }
}
