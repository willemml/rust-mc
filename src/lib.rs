#![feature(array_chunks)]

/// Minecraft implementations.
pub mod minecraft;
/// Mojang API implementation.
pub mod mojang;

pub use minecraft::{client::Client, server::MinecraftServer, status::StatusChecker};
pub use mojang::auth;

use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::sync::Arc;
use tokio::{
    runtime::Runtime,
    sync::{mpsc, Mutex},
    task::JoinHandle,
};

/// Async main, used for launching and testing server and optional client.
async fn async_main(runtime: Arc<Mutex<Runtime>>, init_client: bool) {
    if init_client {
        let client = start_client(Ipv4Addr::LOCALHOST, 25565, "rust_mc", runtime.clone()).await;
        if let Ok((_, client, _)) = client {
            println!("Successfully connected to localhost:25565");
            let mut buffer = String::new();
            let stdin = std::io::stdin();
            loop {
                if let Ok(_) = stdin.read_line(&mut buffer) {
                    if let Err(_) = client.lock().await.send_chat_message(&buffer).await {
                        println!("Failed to send message...")
                    }
                    buffer.clear();
                }
            }
        } else {
            println!("Client failed to connect: {}", client.err().unwrap())
        }
    }
}

type ClientResult = anyhow::Result<(JoinHandle<()>, Arc<Mutex<Client>>, mpsc::Sender<()>)>;

/// Starts a client, mostly for testing.
async fn start_client(
    ip: Ipv4Addr,
    port: u16,
    username: &str,
    runtime: Arc<Mutex<Runtime>>,
) -> ClientResult {
    let client = Arc::new(Mutex::new(Client::new(
        SocketAddr::new(IpAddr::V4(ip), port),
        auth::Profile::new(username, "", true),
    )));
    let (tx, rx) = mpsc::channel(20);
    let connect = client.lock().await.connect().await;
    if let Ok(_) = connect {
        Ok((
            runtime
                .lock()
                .await
                .spawn(Client::start_loop(client.clone(), rx)),
            client,
            tx,
        ))
    } else {
        Err(connect.err().unwrap())
    }
}
