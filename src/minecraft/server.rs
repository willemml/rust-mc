use std::sync::Arc;

use tokio::{sync::Mutex, net::TcpListener};
use mcproto_rs::protocol::PacketDirection;

use super::net::connection::MinecraftConnection;

pub struct Server {
    connections: Arc<Mutex<Vec<MinecraftConnection>>>,
    bind_address: String,
}

impl Server {
    pub fn new(bind_address: String) -> Self {
        Server {
            connections: Arc::new(Mutex::new(vec![])),
            bind_address
        }
    }

    pub fn start(&mut self) {
        let connections = self.connections.clone();
        let address = self.bind_address.clone();
        let listener = async move {
            let mut listener = TcpListener::bind(address).await;
            if let Ok(listener) = &mut listener {
                loop {
                    if let Ok((socket, address)) = listener.accept().await {
                        println!("Incoming connection from {}.", address.to_string());
                        let mut client = MinecraftConnection::from_tcp_stream(socket);
                        client.handshake(None, None, None).await;
                        connections.lock().await.push(client);
                    }
                }
            }
        };
        tokio::runtime::Runtime::new().unwrap().block_on(listener);
    }
}
