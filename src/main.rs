#![feature(array_chunks)]

pub mod minecraft;
pub mod mojang;

pub use minecraft::client::Client;
pub use mojang::auth;

use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::sync::Arc;
use tokio::sync::{mpsc, Mutex};

fn main() {
    let status_checker =
        minecraft::status::StatusChecker::new(IpAddr::V4(Ipv4Addr::LOCALHOST), 25565);
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
        auth::Profile::new("test_player", "", true),
    );
    let connect = client.connect().await;
    let client_arc = Arc::new(Mutex::new(client));
    let client_other = client_arc.clone();
    let (_tx, rx) = mpsc::channel(20);
    if let Ok(_) = connect {
        println!("test_player connected successfully!");
        std::thread::sleep(std::time::Duration::from_millis(100));
        client_arc
            .lock()
            .await
            .send_chat_message("Hi, I am test_player!".to_string())
            .await;
        //std::thread::spawn(|| {
            minecraft::net::scanner::PacketScanner::start(client_other, Arc::new(Mutex::new(rx))).await;
        //});
    } else {
        println!("test_player failed to connect, {}", connect.err().unwrap())
    }
    loop {
        // wait for exit
        std::thread::sleep(std::time::Duration::from_millis(100))
    }
}
