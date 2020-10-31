fn main() {
    let handshake_spec = mcproto_rs::v1_16_3::HandshakeSpec {
        version: mcproto_rs::types::VarInt::from(735),
        server_address: "127.0.0.1".to_owned(),
        server_port: 25565,
        next_state: mcproto_rs::v1_16_3::HandshakeNextState::Status
    };
    let handshake_packet = mcproto_rs::v1_16_3::Packet753::Handshake(handshake_spec);
    let connector = async { 
        println!("Connecting...");
        if let Ok(mut server) = connect().await {
            println!("Sending handshake packets...");
            if let Ok(_) = server.write_packet(handshake_packet).await {
                println!("Sent handshake packet successfully.");
            };
            if let Ok(_) = server.write_packet(mcproto_rs::v1_16_3::Packet753::StatusRequest(mcproto_rs::v1_16_3::StatusRequestSpec {})).await {
                println!("Sent request packet successfully.");
            };
            println!("Receiving response...");
            if let Ok(response) = server.read_packet::<mcproto_rs::v1_16_3::RawPacket753>().await {
                use mcproto_rs::Deserialize;
                match response {
                    Some(packet) => {
                        match packet {
                            mcproto_rs::v1_16_3::RawPacket753::StatusResponse(body) => {
                                if let Ok(status) = mcproto_rs::status::StatusSpec::mc_deserialize(body.data) {
                                    println!("{:?}", status.value);
                                } else {
                                    println!("Bad packet data.");
                                };
                            },
                            _other => println!("Wrong packet.")
                        };
                    },
                    None => println!("No packet."),
                };
            } else {
                println!("Failed to read response.");
            };
        } else {
            println!("Failed to connect to server.")
        };
    };
    tokio::runtime::Runtime::new().unwrap().block_on(connector);
}

async fn connect() -> tokio::io::Result<mctokio::TcpConnection> {
    let connection = mctokio::TcpConnection::connect_to_server(std::net::SocketAddrV4::new(
        std::net::Ipv4Addr::new(127, 0, 0, 1),
        25565,
    ));
    Ok(connection.await?)
}
