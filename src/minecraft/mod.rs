/// Client for Minecraft servers.
pub mod client;
/// Minecraft server status checking.
pub mod status;
/// Minecraft networking.
pub mod net;

use mcproto_rs::v1_16_3 as proto;
use proto::{Packet753 as Packet, RawPacket753 as RawPacket};
