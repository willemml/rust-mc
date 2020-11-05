use crate::net::{Packet, ServerConnection};
use crate::proto::HandshakeNextState;
use mcproto_rs::status::StatusSpec;
use mctokio::TcpConnection;
use std::net::{IpAddr, SocketAddr};

pub struct StatusChecker {
    address: IpAddr,
    port: u16,
}

impl StatusChecker {
    pub fn get_status_sync(&self) -> Result<StatusSpec, anyhow::Error> {
        let status = tokio::runtime::Runtime::new()
        .unwrap()
        .block_on(self.get_status());
        return if let Ok(status) = status 
        {
            Ok(status)
        } else {
            Err(status.err().unwrap_or(anyhow::anyhow!("Failed to get status.".to_string())))
        }
    }

    pub async fn get_status(&self) -> Result<mcproto_rs::status::StatusSpec, anyhow::Error> {
        let address = SocketAddr::new(self.address, self.port);
        let connection = TcpConnection::connect_to_server(address).await;
        if let Ok(connected) = connection {
            let mut server = ServerConnection::from_tcp_connection(connected);
            let handshake = server.handshake(HandshakeNextState::Status).await;
            if let Ok(_) = &handshake {
                let packet = server.must_read_next_packet().await;
                match packet {
                    Packet::StatusResponse(body) => {
                        return Ok(body.response);
                    }
                    _ => {
                        return Err(anyhow::anyhow!("Wrong packet"));
                    }
                }
            }
        };
        return Err(anyhow::anyhow!("Error sending packets."));
    }
}
