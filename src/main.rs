#![feature(array_chunks)]

pub mod handler;
pub mod net;
pub mod scanner;
pub mod status;
pub mod client;

pub use mcproto_rs::v1_16_3 as proto;

use crate::scanner::PacketScanner;
use crate::net::{ServerConnection, Packet};
use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use crate::handler::PacketHandler;

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
    let mut client = client::MinecraftClient::new(SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), 25565), "test".to_string());
    let connect = client.connect().await;
    if let Ok(_) = connect {
        println!("Connected successfully!")
    } else {
        println!("Connection failed, {}", connect.err().unwrap())
    }
}

pub struct TestHandler;

impl PacketHandler for TestHandler {
    fn handle_packet(&mut self, _packet: Packet, scanner: &mut PacketScanner) {
        println!("Received a packet!");
        scanner.stop();
    }
}


