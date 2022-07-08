use super::connection::MinecraftConnection;
use super::{
    proto::{HandshakeNextState, StatusResponseSpec},
    Packet,
};
use mcproto_rs::status::{StatusFaviconSpec, StatusPlayersSpec, StatusSpec, StatusVersionSpec};
use mcproto_rs::types::Chat;
use std::net::{IpAddr, SocketAddr};

/// A status checker for Minecraft servers.
pub struct StatusChecker {
    /// IP address of the server to get the status of.
    address: IpAddr,
    /// Port of the server to get the status of.
    port: u16,
}

impl StatusChecker {
    /// Returns a StatusChecker for `address`:`port`.
    ///
    /// # Arguments
    ///
    /// * `address` IP of the new StatusChecker should attach to.
    /// * `port` Port of the new StatusChecker should attach to.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use std::net::{IpAddr, Ipv4Addr, SocketAddr};
    /// use rust_mc::minecraft::status::StatusChecker;
    ///
    /// let status_checker = minecraft::status::StatusChecker::new(IpAddr::V4(Ipv4Addr::LOCALHOST), 25565);
    /// ```
    pub fn new(address: IpAddr, port: u16) -> Self {
        Self { address, port }
    }

    /// Get the status of the attached server asynchronously.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use std::net::{IpAddr, Ipv4Addr, SocketAddr};
    /// use futures::executor::block_on;
    /// use rust_mc::minecraft::status::StatusChecker;
    ///
    /// let status_checker = minecraft::status::StatusChecker::new(IpAddr::V4(Ipv4Addr::LOCALHOST), 25565);
    ///
    /// let status = async {
    ///     if let Ok(status) = status_checker.get_status().await {   
    ///         println!("Server description: {}", status.description.to_traditional().unwrap().to_string());
    ///     }
    /// }
    ///
    /// block_on(status);
    /// ```
    pub async fn get_status(&self) -> Result<StatusSpec, anyhow::Error> {
        let address = SocketAddr::new(self.address, self.port);
        let mut connection = MinecraftConnection::connect_async(address).await;
        if let Ok(server) = &mut connection {
            let handshake = server
                .handshake(Some(HandshakeNextState::Status), None)
                .await;
            if let Ok(_) = &handshake {
                let packet = server.read_next_packet().await;
                if let Ok(packet) = packet {
                    match packet.unwrap() {
                        Packet::StatusResponse(body) => {
                            return Ok(body.response);
                        }
                        _ => return Err(anyhow::anyhow!("Wrong packet.")),
                    }
                } else {
                    return Err(packet.unwrap_err());
                }
            }
        };
        return Err(anyhow::anyhow!("Error sending packets."));
    }

    /// Get the status of the attached server in a synchronous fashion.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use std::net::{IpAddr, Ipv4Addr, SocketAddr};
    /// use futures::executor::block_on;
    /// use rust_mc::minecraft::status::StatusChecker;
    ///
    /// let status_checker = minecraft::status::StatusChecker::new(IpAddr::V4(Ipv4Addr::LOCALHOST), 25565);
    ///
    /// if let Ok(status) = status_checker.get_status_sync() {   
    ///     println!("Server description: {}", status.description.to_traditional().unwrap().to_string());
    /// }
    /// ```
    pub fn get_status_sync(&self) -> Result<StatusSpec, anyhow::Error> {
        let status = tokio::runtime::Runtime::new()
            .unwrap()
            .block_on(self.get_status());
        return if let Ok(status) = status {
            Ok(status)
        } else {
            Err(status
                .err()
                .unwrap_or(anyhow::anyhow!("Failed to get status.".to_string())))
        };
    }
}

/// Server status manager for servers.
pub struct ServerStatus {
    /// Description/MOTD to send clients.
    pub description: Chat,
    /// The version spec to send clients, contains the version string and the protocol version number.
    pub version: StatusVersionSpec,
    /// Player count and short list to send to clients.
    pub players: StatusPlayersSpec,
    /// Server icon to send to clients.
    pub favicon: Option<StatusFaviconSpec>,
}

impl ServerStatus {
    /// Sends this Status object to a Client.
    ///
    /// # Arguments
    ///
    /// * `client` Arc-Mutex containing the client to send the status to.
    pub async fn send_status(&self, client: &mut MinecraftConnection) -> anyhow::Result<()> {
        let status_spec = StatusSpec {
            description: self.description.clone(),
            favicon: self.favicon.clone(),
            players: self.players.clone(),
            version: Some(self.version.clone()),
        };
        let response_spec = StatusResponseSpec {
            response: status_spec,
        };
        client
            .write_packet(super::Packet::StatusResponse(response_spec))
            .await
    }
}
