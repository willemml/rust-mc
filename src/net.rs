use mcproto_rs::protocol::State;
use mctokio::{Bridge, TcpReadBridge, TcpWriteBridge, TcpConnection};
use std::net::SocketAddr;
use crate::proto;
pub use proto::{Packet753 as Packet, RawPacket753 as RawPacket};

pub struct ServerConnection {
    reader: TcpReadBridge,
    writer: TcpWriteBridge,
}

impl ServerConnection {
    pub fn new(reader: TcpReadBridge, writer: TcpWriteBridge) -> Self{
        return Self {
            reader,
            writer,
        };
    }

    pub async fn connect_async(address: SocketAddr) -> Result<Self, std::io::Error> {
        let connection = TcpConnection::connect_to_server(address).await;
        if let Ok(connected) = connection {
            Ok(ServerConnection::from_tcp_connection(connected))
        } else {
            Err(connection.err().unwrap())
        }
    }

    pub fn connect(address: SocketAddr) -> Result<Self, std::io::Error> {
        tokio::runtime::Runtime::new().unwrap().block_on(Self::connect_async(address))
    }

    pub fn from_tcp_connection(connection: TcpConnection) -> Self {
        Self::new(connection.reader, connection.writer)
    }

    pub async fn handshake(
        &mut self,
        next_state: proto::HandshakeNextState,
        name: &String
    ) -> anyhow::Result<()> {
        let handshake = proto::HandshakeSpec {
            version: mcproto_rs::types::VarInt::from(753),
            server_address: "".to_string(),
            server_port: 25565,
            next_state: next_state.clone(),
        };
        if let Err(error) = self.write_packet(Packet::Handshake(handshake)).await {
            return Err(error);
        } else {
            if next_state == proto::HandshakeNextState::Status {
                self.set_state(State::Status);
                if let Err(error) = self
                    .write_packet(Packet::StatusRequest(proto::StatusRequestSpec {}))
                    .await
                {
                    return Err(error);
                }
            } else {
                self.set_state(State::Login);
                if let Err(error) = self
                    .write_packet(Packet::LoginStart(proto::LoginStartSpec { name: name.clone() }))
                    .await
                {
                    return Err(error);
                }
            };
            return Ok(());
        };
    }

    pub async fn write_packet(&mut self, packet: Packet) -> anyhow::Result<()> {
        self.writer.write_packet(packet).await
    }

    pub fn set_state(&mut self, state: State) {
        self.reader.set_state(state.clone());
        self.writer.set_state(state);
    }

    pub fn enable_encryption(&mut self, key: &[u8], iv: &[u8]) -> anyhow::Result<()> {
        let reader = self.reader.enable_encryption(key, iv);
        if let Err(error) = reader {
            Err(error)
        } else {
            self.writer.enable_encryption(key, iv)
        }
    }

    pub fn set_compression_threshold(&mut self, threshold: i32) {
        self.reader.set_compression_threshold(Some(threshold));
        self.writer.set_compression_threshold(Some(threshold));
    }

    pub async fn read_next_packet(&mut self) -> anyhow::Result<Option<Packet>> {
        if let Some(raw) = self.reader.read_packet::<RawPacket>().await? {
            Ok(Some(mcproto_rs::protocol::RawPacket::deserialize(&raw)?))
        } else {
            Ok(None)
        }
    }
}
