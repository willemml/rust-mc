#![feature(array_chunks)]

/// Minecraft implementations.
pub mod minecraft;
/// Mojang API implementation.
pub mod mojang;

pub use minecraft::{client::Client, server::Server, status::StatusChecker};
pub use mojang::auth;

use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::sync::Arc;
use tokio::{
    runtime::Runtime,
    sync::{mpsc, Mutex},
    task::JoinHandle,
};

/// Main, currently just used to create runtimes start `async_main`.
fn main() {
    Runtime::new()
        .unwrap()
        .block_on(async_main(Arc::new(Mutex::new(Runtime::new().unwrap()))));
}

/// Async main, used for launching and testing client and server.
#[allow(unused_must_use)]
async fn async_main(runtime: Arc<Mutex<Runtime>>) {
    let server = start_server("127.0.0.1:25565".to_string(), "Rust MC server testing.".to_string(), false, runtime.clone()).await;
    let client = start_client(Ipv4Addr::LOCALHOST, 25565, "wnuke", runtime.clone()).await;
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
    server.0.await;
}

/// Starts a server, mostly for testing.
async fn start_server(
    address: String,
    description: String,
    online: bool,
    runtime: Arc<Mutex<Runtime>>,
) -> (
    JoinHandle<anyhow::Result<()>>,
    Arc<Mutex<Server>>,
    mpsc::Sender<()>,
) {
    let server = Arc::new(Mutex::new(Server::new(address, description, 5, online)));
    let (tx, rx) = mpsc::channel(20);
    (
        runtime
            .lock()
            .await
            .spawn(Server::start(server.clone(), rx, runtime.clone())),
        server,
        tx,
    )
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
