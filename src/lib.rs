#![feature(array_chunks)]

/// Minecraft implementations.
pub mod minecraft;
/// Mojang API implementation.
pub mod mojang;

pub use minecraft::{client::MinecraftClient, server::MinecraftServer, status::StatusChecker};
pub use mojang::auth;
