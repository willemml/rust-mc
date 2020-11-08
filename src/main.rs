#![feature(array_chunks)]

pub mod minecraft;
pub mod mojang;

pub use minecraft::client::Client;
pub use mojang::auth;

use std::sync::Arc;
use std::net::{IpAddr, Ipv4Addr, SocketAddr};
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
        Client::start_loop(client_other, rx);
        let mut buffer = String::new();
        let stdin = std::io::stdin();
        loop {
            if let Ok(_) = stdin.read_line(&mut buffer) {
                client_arc.lock().await.send_chat_message(&buffer).await;
                buffer.clear();
            }
        }
    } else {
        println!("test_player failed to connect, {}", connect.err().unwrap())
    }
}
