/// Client for Minecraft servers.
pub mod client;
/// Minecraft server implementation
pub mod server;
/// Minecraft server status checking.
pub mod status;
/// mctokio Minecraft networking wrapper.
pub mod connection;

use mcproto_rs::v1_16_3 as proto;
use proto::{Packet753 as Packet, RawPacket753 as RawPacket};
