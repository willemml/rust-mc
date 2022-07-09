use futures::executor::block_on;
use rust_mc::{self, MinecraftServer};

fn main() {
    block_on(async_main());
}

async fn async_main() {
    let (server, _tx) = MinecraftServer::new(
        "127.0.0.1:25565".to_string(),
        "Rust test MC server".to_string(),
        5,
        true,
    );
    let server_handle = server.start().await.unwrap();
    server_handle.await.unwrap();
}
