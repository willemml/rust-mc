use std::net::{SocketAddr, IpAddr, Ipv4Addr};

mod net;

fn main() {
    let address = SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), 25565);
    tokio::runtime::Runtime::new().unwrap().block_on(connect(address));
}

async fn connect(address: SocketAddr) {
    let connection = mctokio::TcpConnection::connect_to_server(address).await;
    if let Ok(connected) = connection {
        let mut server = net::ServerConnection { reader: connected.reader, writer: connected.writer };
    };
}


