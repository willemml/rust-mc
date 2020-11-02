use mcproto_rs::Deserialize;

fn main() {
    let fifty_millis = std::time::Duration::from_millis(50);
    let handshake_spec = mcproto_rs::v1_16_3::HandshakeSpec {
        version: mcproto_rs::types::VarInt::from(753),
        server_address: "127.0.0.1".to_owned(),
        server_port: 25565,
        next_state: mcproto_rs::v1_16_3::HandshakeNextState::Status,
    };
    let handshake_packet = mcproto_rs::v1_16_3::Packet753::Handshake(handshake_spec);
    let mut server_connection = ServerConnection::new(std::net::SocketAddr::new(
        std::net::IpAddr::V4(std::net::Ipv4Addr::new(127, 0, 0, 1)),
        25565,
    ));
    if server_connection.send_packet(handshake_packet) {
        println!("Sent handshake packet.");
    } else {
        println!("Failed to send handshake packet.");
    }
    std::thread::sleep(std::time::Duration::from_millis(50));
    server_connection.send_packet(mcproto_rs::v1_16_3::Packet753::StatusRequest(
        mcproto_rs::v1_16_3::StatusRequestSpec {},
    ));
    std::thread::sleep(fifty_millis);
    println!("Sent packets.");
    loop {
        if let Ok(raw_packet) =
            server_connection.read_raw_packet::<mcproto_rs::v1_16_3::RawPacket753>()
        {
            println!("Reading packet...");
            match raw_packet {
                mcproto_rs::v1_16_3::RawPacket753::StatusResponse(body) => {
                    if let Ok(response) =
                        mcproto_rs::v1_16_3::StatusResponseSpec::mc_deserialize(body.data)
                    {
                        println!(
                            "Description: {}",
                            response
                                .value
                                .response
                                .description
                                .to_traditional()
                                .unwrap_or("none".to_owned())
                        );
                    };
                }
                _ => {}
            };
        };
        std::thread::sleep(fifty_millis);
    }
}

struct ServerConnection {
    server_connection: mctokio::TcpConnection,
}

impl ServerConnection {
    fn new(address: std::net::SocketAddr) -> Self {
        let future = async move {
            let connection = mctokio::TcpConnection::connect_to_server(address);
            connection.await
        };
        let connection = tokio::runtime::Runtime::new().unwrap().block_on(future);
        if let Ok(connected) = connection {
            return Self {
                server_connection: connected,
            };
        } else {
            panic!("Failed to connect to server.");
        };
    }

    fn read_raw_packet<'a, P>(&'a mut self) -> Result<P, String>
    where
        P: mcproto_rs::protocol::RawPacket<'a>,
    {
        let future = async move {
            let read = self.server_connection.read_packet::<P>();
            read.await
        };
        let thing = tokio::runtime::Runtime::new().unwrap().block_on(future);
        return if let Ok(read) = thing {
            match read {
                Some(packet) => Ok(packet),
                None => Err("Empty packet.".to_owned()),
            }
        } else {
            println!("{:?}", thing.err());
            Err("Packet read failed.".to_owned())
        };
    }

    fn send_packet<P>(&mut self, packet: P) -> bool
    where
        P: mcproto_rs::protocol::Packet,
    {
        let future = async move {
            let write = self.server_connection.write_packet(packet);
            write.await
        };
        let thing = tokio::runtime::Runtime::new().unwrap().block_on(future);
        if let Ok(_write) = &thing {
            return true;
        } else {
            println!("{:?}", &thing.err());
            return false;
        };
    }
}
