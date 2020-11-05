pub mod handler;
pub mod net;
pub mod scanner;
pub mod status;

pub use mcproto_rs::v1_16_3 as proto;

use crate::scanner::{PacketScanner, TestHandler};
use crate::net::ServerConnection;
use std::net::{IpAddr, Ipv4Addr};

fn main() {
    let status_checker = status::StatusChecker::new(IpAddr::V4(Ipv4Addr::LOCALHOST), 25565);
    if let Ok(status) = status_checker.get_status_sync() {
        println!("Description: {}", status.description.to_traditional().unwrap().to_string());
    }
    tokio::runtime::Runtime::new().unwrap().block_on(async {
        async_main().await
    });
}

async fn async_main() {
    let connecting = ServerConnection::connect_async(IpAddr::V4(Ipv4Addr::LOCALHOST), 25565).await;
    if let Ok(mut connection) = connecting {
        connection.handshake(crate::proto::HandshakeNextState::Status).await;
        let mut scanner = PacketScanner::new(connection, TestHandler {});
        scanner.start().await;
    }
}


