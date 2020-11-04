use std::net::{SocketAddr, IpAddr, Ipv4Addr};

mod net;

fn main() {
    println!("Getting status of localhost:25565...");
    let address = SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), 25565);
    if let Ok(status) = tokio::runtime::Runtime::new().unwrap().block_on(get_status(address)) {
        println!("Status: {:?}", status);
    } else {
        println!("Failed to get status of localhost::25565.");
    };
}

async fn get_status(address: SocketAddr) -> Result<mcproto_rs::status::StatusSpec, anyhow::Error> {
    let connection = mctokio::TcpConnection::connect_to_server(address).await;
    if let Ok(connected) = connection {
        let mut server = net::ServerConnection::new(connected.reader, connected.writer);
        let handshake = server.handshake(net::proto::HandshakeNextState::Status).await;
        if let Ok(_) = &handshake {
            let packet = server.must_read_next_packet().await;
            match packet {
                net::Packet::StatusResponse(body) => {
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


