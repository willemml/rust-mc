use futures::executor::block_on;
use rust_mc::{self, MinecraftServer, MinecraftClient};
use rust_mc::mojang::auth;

fn main() {
    block_on(async_main());
}

async fn async_main() {
    let (server, _tx) = MinecraftServer::new(
        "127.0.0.1:25565".to_string(),
        "Rust test MC server".to_string(),
        5,
        false,
    );
    let server_handle = server.start().await.unwrap();

    server.on_receive_packet(|p| {
        println!("[Listener1] Packet received: {:?}", p);
    }).await;

    let mut client = MinecraftClient::new(
        "127.0.0.1:25565".parse().unwrap(),
        auth::Profile::new("TestUser", "", true),
    );
    let (_handle, _txc) = client.connect().await.unwrap();
    client.send_chat_message("Test message").await;
    println!("Finish sending chat message");

    server_handle.await.unwrap();
}
