use std::net::SocketAddr;
use crate::net::ServerConnection;
use crate::net::Packet;
use mcproto_rs::protocol::State;

pub struct MinecraftClient {
    address: SocketAddr,
    username: String,
    server: Option<ServerConnection>
}

impl MinecraftClient {
    pub fn new(address: SocketAddr, username: String) -> Self {
        MinecraftClient {
            address,
            username,
            server: None
        }
    }

    pub async fn connect(&mut self) {
        if let Ok(connection) = ServerConnection::connect_async(self.address).await {
            self.server = Some(connection);
            if let Some(server) = &mut self.server {
                if let Ok(_) = server.handshake(crate::proto::HandshakeNextState::Login, &self.username).await {
                    self.login();
                }
            }
        } else {
            self.server = None;
        }
    }

    async fn read_packet(&mut self) -> Result<Packet, anyhow::Error> {
        if let Some(server) = &mut self.server {
            let read = server.read_next_packet().await;
            if let Ok(possible_packet) = read {
                if let Some(packet) = possible_packet {
                    Ok(packet)
                } else {
                    Err(anyhow::anyhow!("Empty packet."))
                }
            } else {
                Err(read.err().unwrap())
            }
        } else {
            Err(anyhow::anyhow!("Not connected."))
        }
    }

    async fn set_state(&mut self, new_state: State) -> Result<(), anyhow::Error> {
        if let Some(server) = &mut self.server {
            server.set_state(new_state);
            Ok(())
        } else {
            Err(anyhow::anyhow!("Not connected."))
        }
    }

    pub async fn login(&mut self) -> Result<(), anyhow::Error> {
        let read = self.read_packet().await;
        if let Ok(packet) = read {
            match packet {
                Packet::LoginEncryptionRequest(body) => {
                    Err(anyhow::anyhow!("Can't do encryption yet."))
                }
                Packet::LoginSetCompression(body) => {
                    Err(anyhow::anyhow!("Can't do compression yet."))
                }
                Packet::LoginSuccess(body) => {
                    self.set_state(State::Play).await
                }
                _ => {
                    Err(anyhow::anyhow!("Wrong packet."))
                }
            }
        } else {
            Err(read.err().unwrap())
        }
    }
}
