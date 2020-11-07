#![feature(array_chunks)]

pub mod mojang;
pub mod minecraft;

pub use mojang::auth;
pub use minecraft::client::Client;

use std::net::{IpAddr, Ipv4Addr, SocketAddr};

fn main() {
    let status_checker = minecraft::status::StatusChecker::new(IpAddr::V4(Ipv4Addr::LOCALHOST), 25565);
    if let Ok(status) = status_checker.get_status_sync() {
        println!(
            "Description: {}",
            status.description.to_traditional().unwrap().to_string()
        );
    }
    tokio::runtime::Runtime::new()
        .unwrap()
        .block_on(async { async_main().await });
}

async fn async_main() {
    let mut client = Client::new(
        SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), 25565),
        auth::Profile::new("test", "", true),
    );
    let connect = client.connect().await;
    if let Ok(_) = connect {
        println!("Connected successfully!")
    } else {
        println!("Connection failed, {}", connect.err().unwrap())
    }
}
