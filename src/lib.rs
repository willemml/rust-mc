#![feature(array_chunks)]
#[macro_use]
extern crate log;

/// Minecraft implementations.
pub mod minecraft;
/// Mojang API implementation.
pub mod mojang;

pub use minecraft::{client::MinecraftClient, server::MinecraftServer, status::StatusChecker};
pub use mojang::auth;
