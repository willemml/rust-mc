use rust_mc::{MinecraftServer, MinecraftClient};
use rust_mc::mojang::auth;
use tokio::{self, join};

#[macro_use]
extern crate log;

#[tokio::main]
async fn main() {
    env_logger::init();

    let (server, _tx) = MinecraftServer::new(
        "127.0.0.1:25565",
        "Rust test MC server",
        5,
        false,
    );
    let server_handle = server.start().await.unwrap();

    let mut server_packet_receiver = server.add_packet_receiver().await;

    
    let mut client = MinecraftClient::new(
        "127.0.0.1:25565".parse().unwrap(),
        auth::Profile::new("TestUser", "", true),
    );
    let (_handle, _txc) = client.connect().await.unwrap();
    let mut client_packet_listener = client.add_packet_receiver().await;
    client.send_chat_message("Test message").await;
    
    // Listen to packets from the server
    let on_server_packet_loop = async move {
        loop {
            if let Ok(p) = server_packet_receiver.recv().await {
                info!("[Server List] Packet received: {:?}", p);   
            };
        }
    };
    // Listen to packets from the client
    let on_client_packet_loop = async move {
        loop {
            if let Ok(p) = client_packet_listener.recv().await {
                info!("[Client List] Packet received: {:?}", p);   
            };
        }
    };
    
    // Run both listeners in the same thread concurrently
    join!(on_server_packet_loop, on_client_packet_loop);

    server_handle.await.unwrap();
}
