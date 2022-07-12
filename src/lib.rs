#![feature(array_chunks)]
#[macro_use]
extern crate log;

/// Minecraft implementations.
pub mod minecraft;
/// Mojang API implementation.
pub mod mojang;

pub use minecraft::{client::MinecraftClient, server::MinecraftServer, status::StatusChecker};
pub use mojang::auth;

/// Initialize the library. Required before using any components
pub fn init() {
    env_logger::init();
    info!("[rust-mc] Initialized!");
}
