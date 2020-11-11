#![feature(array_chunks)]

/// Minecraft implementations.
pub mod minecraft;
/// Mojang API implementation.
pub mod mojang;

pub use minecraft::{client::Client, server::Server};
pub use mojang::auth;

use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::sync::Arc;
use tokio::sync::{mpsc, Mutex};

fn main() {
    std::thread::sleep(std::time::Duration::from_millis(100));
    let mut runtime = tokio::runtime::Runtime::new().unwrap();
    runtime.spawn(async { Server::new("127.0.0.1:25565".to_string()).start().await });
    let status_checker =
        minecraft::status::StatusChecker::new(IpAddr::V4(Ipv4Addr::LOCALHOST), 25565);
    if let Ok(status) = status_checker.get_status_sync() {
        println!(
            "Description: {}",
            status.description.to_traditional().unwrap().to_string()
        );
    }
    runtime.block_on(async_main(tokio::runtime::Runtime::new().unwrap()));
}

async fn async_main(runtime: tokio::runtime::Runtime) {
    let mut client = Client::new(
        SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), 25565),
        auth::Profile::new("test_player", "", true),
    );
    let connect = client.connect().await;
    let client_arc = Arc::new(Mutex::new(client));
    let client_other = client_arc.clone();
    let (_tx, rx) = mpsc::channel(20);
    if let Ok(_) = connect {
        println!("Successfully connected to localhost:25565");
        runtime.spawn(Client::start_loop(client_other, rx));
        let mut buffer = String::new();
        let stdin = std::io::stdin();
        loop {
            if let Ok(_) = stdin.read_line(&mut buffer) {
                if let Err(_) = client_arc.lock().await.send_chat_message(&buffer).await {
                    println!("Failed to send message...")
                }
                buffer.clear();
            }
        }
    } else {
        println!("test_player failed to connect, {}", connect.err().unwrap())
    }
}
