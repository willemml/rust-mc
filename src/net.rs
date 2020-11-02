use mcproto_rs::v1_16_3 as proto;
use proto::{Packet753 as Packet, RawPacket753 as RawPacket};

struct ServerConnection {
    reader: mctokio::TcpReadBridge,
    writer: mctokio::TcpWriteBridge,
}

impl ServerConnection {
    async fn handshake(&self, next_state: proto::HandshakeNextState) {
        let handshake = proto::HandshakeSpec {
            version: 753,
            address: "".to_owned,
            port: 25565,
            next_state: next_state.clone(),
        };
    }

    async fn write_packet(&self, packet: Packet) -> Result<()> {
        self.writer.write_packet(packet).await
    }

    async fn write_raw_packet(&self, raw_packet: RawPacket) -> Result<()> {
        self.writer.write_raw_packet(raw_packet).await
    }

    async fn must_read_next_packet(&self) -> Result<Packet> {
        if let Some(Packet) = self.read_next_packet().await? {
            Ok(packet)
        } else {
            Err("Exepected a Packet, got EOF.")
        }
    }

    async fn read_next_packet(&self) -> Result<Option<Packet>> {
        if let Some(raw) = self.reader.read_packet::<RawPacket>().await? {
            Ok(Some(raw.deserialize()?))
        } else {
            Ok(None)
        }
    }
}
