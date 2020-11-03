use mcproto_rs::v1_16_3 as proto;
use proto::{Packet753 as Packet, RawPacket753 as RawPacket};

pub struct ServerConnection {
    reader: mctokio::TcpReadBridge,
    writer: mctokio::TcpWriteBridge,
}

impl ServerConnection {
    async fn handshake(&self, next_state: proto::HandshakeNextState) {
        let handshake = proto::HandshakeSpec {
            version: mcproto_rs::types::VarInt::from(753),
            server_address: "".to_string(),
            server_port: 25565,
            next_state: next_state.clone(),
        };
        self.write_packet(Packet::Handshake(handshake));
        if next_state == proto::HandshakeNextState::Status {
            self.write_packet(Packet::StatusRequest(proto::StatusRequestSpec {} ));
        };
    }

    async fn write_packet(&mut self, packet: Packet) -> Result<()> {
        self.writer.write_packet(packet).await
    }

    async fn write_raw_packet(&mut self, raw_packet: RawPacket) -> Result<()> {
        self.writer.write_raw_packet(raw_packet).await;
    }

    async fn must_read_next_packet(&self) -> Result<Packet> {
        return if let Some(packet) = self.read_next_packet().await? {
            Ok(packet)
        } else {
            Err("Exepected a Packet, got EOF.")
        }
    }

    async fn read_next_packet(&self) -> Result<Option<Packet>> {
        if let Some(raw) = self.reader.read_packet::<RawPacket>().await? {
            Ok(Some(mcproto_rs::protocol::RawPacket::deserialize(&raw)?))
        } else {
            Ok(None)
        }
    }
}
