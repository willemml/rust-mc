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

    let mut client = MinecraftClient::new(
        "127.0.0.1:25565".parse().unwrap(),
        auth::Profile::new("TestUser", "", true),
    );
    client.connect().await.unwrap();
    client.send_chat_message("Test message").await;

    server_handle.await.unwrap();
}
