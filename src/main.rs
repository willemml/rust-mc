use futures::executor::block_on;
use rust_mc::{self, MinecraftServer, MinecraftClient};
use rust_mc::mojang::auth;

#[macro_use]
extern crate log;

fn main() {
    rust_mc::init();
    block_on(async_main());
}

async fn async_main() {
    let (server, _tx) = MinecraftServer::new(
        "127.0.0.1:25565",
        "Rust test MC server",
        5,
        false,
    );
    let server_handle = server.start().await.unwrap();

    server.on_receive_packet(|p| {
        debug!("[Server List] Packet received: {:?}", p);
    }).await;

    let mut client = MinecraftClient::new(
        "127.0.0.1:25565".parse().unwrap(),
        auth::Profile::new("TestUser", "", true),
    );
    let (_handle, _txc) = client.connect().await.unwrap();
    client.on_receive_packet(|p| {
        debug!("[Client List] Packet received: {:?}", p);
    }).await;
    client.send_chat_message("Test message").await;

    server_handle.await.unwrap();
}
