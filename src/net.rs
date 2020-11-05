use mcproto_rs::protocol::State;
use mctokio::{Bridge, TcpReadBridge, TcpWriteBridge, TcpConnection};
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

    pub fn from_tcp_connection(connection: TcpConnection) -> Self {
        Self::new(connection.reader, connection.writer)
    }

    pub async fn handshake(
        &mut self,
        next_state: proto::HandshakeNextState,
    ) -> Result<(), anyhow::Error> {
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
            };
            return Ok(());
        };
    }

    pub async fn write_packet(&mut self, packet: Packet) -> Result<(), anyhow::Error> {
        self.writer.write_packet(packet).await
    }

    pub fn set_state(&mut self, state: State) {
        self.reader.set_state(state.clone());
        self.writer.set_state(state);
    }

    pub async fn must_read_next_packet(&mut self) -> Packet {
        if let Ok(packet) = self.read_next_packet().await {
            if let Some(packet) = packet {
                return packet;
            } else {
                panic!("Got empty packet on must_read_next_packet.");
            };
        } else {
            panic!("Got EOF instead of packet on must_read_next_packet.");
        };
    }

    pub async fn read_next_packet(&mut self) -> Result<Option<Packet>, anyhow::Error> {
        if let Some(raw) = self.reader.read_packet::<RawPacket>().await? {
            Ok(Some(mcproto_rs::protocol::RawPacket::deserialize(&raw)?))
        } else {
            Ok(None)
        }
    }
}
