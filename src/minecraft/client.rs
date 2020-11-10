use super::{
    net::{connection::MinecraftConnection, handler::PacketHandler},
    proto, Packet,
};
use anyhow::Result;
use mcproto_rs::{protocol::State, types::CountedArray};
use std::net::SocketAddr;

use std::sync::Arc;
use tokio::sync::{
    mpsc::{error::TryRecvError, Receiver},
    Mutex,
};

use std::thread::{spawn, JoinHandle};

/// Error text when the client is not connected to a server.
const SERVER_NONE_ERROR: &str = "Not connected to server.";
/// Error when the client recieves a Packet it was not expecting.
const WRONG_PACKET_ERROR: &str = "Recieved an unexpected packet.";

/// A Minecraft client, a wrapper for the utilities required to connect to a Minecraft server and perform various activities in said server.
pub struct Client {
    /// The socket address of the server the client will connect to.
    address: SocketAddr,
    /// The game profile that will be used to authenticate with the server.
    pub profile: crate::auth::Profile,
    /// When the client is connect this holds the actual connection to the server.
    pub(crate) server: Option<MinecraftConnection>,
    /// Whether or not the client is currently connected to the server.
    connected: bool,
}

impl Client {
    /// Returns a Minecraft client that will connect to the given server with the given profile.
    ///
    /// # Arguments
    ///
    /// * `address` Socket address of the server the client should connect to.
    /// * `profile` Minecraft account/profile that the client should use.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use rust_mc::minecraft::client::Client;
    /// use rust_mc::mojang::auth::Profile;
    /// use std::net::{IpAddr, Ipv4Addr, SocketAddr};
    ///
    /// let mut client = Client::new(
    ///     SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), 25565),
    ///     Profile::new("rust_mc", "", true),
    /// );
    /// ```
    pub fn new(address: SocketAddr, profile: crate::auth::Profile) -> Client {
        Self {
            address,
            profile,
            server: None,
            connected: false,
        }
    }

    /// Tells the client to connect to the Minecraft server at the socket address in it's `address` field.
    ///
    /// # Examples
    ///
    /// This example requires a Minecraft server to be running on localhost:25565.
    ///
    /// ```rust
    /// use rust_mc::minecraft::client::Client;
    /// use rust_mc::mojang::auth::Profile;
    /// use std::net::{IpAddr, Ipv4Addr, SocketAddr};
    /// use futures::executor::block_on;
    ///
    /// let mut client = Client::new(
    ///     SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), 25565),
    ///     Profile::new("rust_mc", "", true),
    /// );
    ///
    /// block_on(client.connect()); // Connect the client.
    /// ```
    pub async fn connect(&mut self) -> Result<()> {
        let auth = self.profile.authenticate().await;
        if let Ok(_) = auth {
            if let Ok(connection) = MinecraftConnection::connect_async(self.address).await {
                self.server = Some(connection);
                if let Some(server) = &mut self.server {
                    if let Ok(_) = server
                        .handshake(
                            Some(proto::HandshakeNextState::Login),
                            None,
                            Some(self.profile.game_profile.name.clone()),
                            None,
                        )
                        .await
                    {
                        self.login().await
                    } else {
                        Err(anyhow::anyhow!("Handshaking with server failed."))
                    }
                } else {
                    Err(anyhow::anyhow!("Disconnected."))
                }
            } else {
                self.server = None;
                Err(anyhow::anyhow!("Failed to connect to server socket."))
            }
        } else {
            Err(auth.err().unwrap())
        }
    }

    /// Spawns a thread that reads packets and tells the client's packet handler about them until told to stop
    /// If the client is not connected the thread will not do anything and will die.
    ///
    /// # Arguments
    ///
    /// * `client` Minecraft client to read packets from.
    /// * `receiver` rx side of a `channel`, when it receives any message or is disconnected the packet read loop is stopped.
    ///
    /// # Examples
    ///
    /// These examples require a Minecraft server to be running on localhost:25565.
    ///
    /// ```rust
    /// use rust_mc::minecraft::client::Client;
    /// use rust_mc::mojang::auth::Profile;
    /// use std::net::{IpAddr, Ipv4Addr, SocketAddr};
    /// use tokio::sync::mpsc::channel;
    /// use futures::executor::block_on;
    ///
    /// let mut client = Client::new(
    ///     SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), 25565),
    ///     Profile::new("rust_mc", "", true),
    /// );
    ///
    /// block_on(client.connect()); // Connect the client before starting the loop.
    /// let (tx, rx) = mpsc::channel(20);
    ///
    /// Client::start_loop(Arc::new(Mutex::new(client)), rx); // Start the packet read loop.
    ///
    /// tx.try_send(()); // Stop the packet read loop.
    /// ```
    ///
    /// If you want to be able to perform operations on the client (such as sending chat messages) from outside of the packet loop and handler:
    ///
    /// ```rust
    /// use rust_mc::minecraft::client::Client;
    /// use rust_mc::mojang::auth::Profile;
    /// use std::net::{IpAddr, Ipv4Addr, SocketAddr};
    /// use tokio::sync::mpsc::channel;
    /// use futures::executor::block_on;
    ///
    /// let mut client = Client::new(
    ///     SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), 25565),
    ///     Profile::new("rust_mc", "", true),
    /// );
    ///
    /// block_on(client.connect()); // Connect the client before starting the loop.
    /// let (tx, rx) = mpsc::channel(20);
    ///
    /// let client_arc = Arc::new(Mutex::new(client)); // Create the client arc mutex before calling the function.
    /// let client_arc_clone = client_arc.clone();  // Clone the client arc before passing it so that you have one in this scope.
    ///
    /// Client::start_loop(client_arc_clone, rx); // Start the packet read loop.
    ///
    /// // Perform some operations that require the client.
    /// let player_name = block_on(client_arc.lock()).profile.game_profile.name.clone();
    /// println!("Client is connected as {}.", player_name);
    ///
    /// tx.try_send(()); // Stop the packet read loop.
    /// ```
    pub fn start_loop(client: Arc<Mutex<Self>>, receiver: Receiver<()>) -> JoinHandle<()> {
        spawn(move || futures::executor::block_on(Self::packet_loop(client, receiver)))
    }

    async fn packet_loop(client: Arc<Mutex<Self>>, receiver: Receiver<()>) {
        let mut receiver = receiver;
        loop {
            if let Err(err) = receiver.try_recv() {
                if err == TryRecvError::Closed {
                    break;
                }
                let client_lock = &mut client.lock().await;
                if !client_lock.connected {
                    break;
                }
                if let Some(server) = &mut client_lock.server {
                    if let Ok(packet) = &mut server.read_next_packet().await {
                        if let Some(packet) = packet {
                            client_lock.handle_packet(packet)
                        }
                    }
                } else {
                    break;
                }
            }
        }
    }

    /// Tells the client to send a chat message
    ///
    /// # Arguments
    ///
    /// * `message` Message the client should send to the server.
    ///
    /// # Examples
    ///
    /// This example requires a Minecraft server to be running on localhost:25565.
    ///
    /// ```rust
    /// use rust_mc::minecraft::client::Client;
    /// use rust_mc::mojang::auth::Profile;
    /// use std::net::{IpAddr, Ipv4Addr, SocketAddr};
    /// use futures::executor::block_on;
    ///
    /// let mut client = Client::new(
    ///     SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), 25565),
    ///     Profile::new("rust_mc", "", true),
    /// );
    ///
    /// block_on(client.connect()); // Connect the client before sending the message.
    ///
    /// block_on(client.send_chat_message("Hello!".to_string())); // Send the chat message.
    /// ```
    pub async fn send_chat_message(&mut self, message: &String) -> Result<()> {
        let spec = proto::PlayClientChatMessageSpec {
            message: message.clone(),
        };
        self.send_packet(Packet::PlayClientChatMessage(spec)).await
    }

    /// Reads a packet from the server when connected.
    async fn read_packet(&mut self) -> Result<Packet> {
        if let Some(server) = &mut self.server {
            let read = server.read_next_packet().await;
            if let Ok(possible_packet) = read {
                if let Some(packet) = possible_packet {
                    Ok(packet)
                } else {
                    Err(anyhow::anyhow!("Empty packet."))
                }
            } else {
                Err(read.err().unwrap())
            }
        } else {
            Err(anyhow::anyhow!(SERVER_NONE_ERROR))
        }
    }

    /// Sends a packet to the server when connected.
    async fn send_packet(&mut self, packet: Packet) -> Result<()> {
        if let Some(server) = &mut self.server {
            server.write_packet(packet).await
        } else {
            Err(anyhow::anyhow!(SERVER_NONE_ERROR))
        }
    }

    /// Sets the client's current state (Handshake, Login, Play, Status), should match server.
    fn set_state(&mut self, new_state: State) -> Result<()> {
        if let Some(server) = &mut self.server {
            server.set_state(new_state);
            Ok(())
        } else {
            Err(anyhow::anyhow!(SERVER_NONE_ERROR))
        }
    }

    /// Sets the size threshold at which packets are compressed before being sent.
    async fn set_compression_threshold(&mut self, threshold: i32) -> Result<()> {
        if self.connected {
            Err(anyhow::anyhow!("Already connected."))
        } else {
            if let Some(server) = &mut self.server {
                server.set_compression_threshold(threshold);
                let read = self.read_packet().await;
                if let Ok(packet) = read {
                    match packet {
                        Packet::LoginSuccess(_) => {
                            self.connected = true;
                            self.set_state(State::Play)
                        }
                        _ => Err(anyhow::anyhow!(WRONG_PACKET_ERROR)),
                    }
                } else {
                    Err(read.err().unwrap())
                }
            } else {
                Err(anyhow::anyhow!(SERVER_NONE_ERROR))
            }
        }
    }

    /// Enables encryption on the client, requires authentication with mojang.
    /// Still WIP, does not actually work.
    async fn enable_encryption(&mut self, spec: proto::LoginEncryptionRequestSpec) -> Result<()> {
        if self.connected {
            return Err(anyhow::anyhow!("Already connected."));
        }
        if self.profile.offline {
            return Err(anyhow::anyhow!(
                "Cannot use encryption with offline account."
            ));
        }
        let key = &spec.public_key.as_slice();
        let token = &spec.verify_token.as_slice();
        if let Some(server) = &mut self.server {
            let buf: &mut [u8] = &mut [0; 16];
            for mut _i in buf.iter() {
                _i = &rand::random::<u8>();
            }
            let response_spec = proto::LoginEncryptionResponseSpec {
                shared_secret: CountedArray::from(buf.to_vec()),
                verify_token: spec.verify_token.clone(),
            };
            let auth = self.profile.join_server(spec.server_id).await;
            if let Ok(_) = auth {
                let respond = server
                    .write_packet(Packet::LoginEncryptionResponse(response_spec))
                    .await;
                if let Ok(_) = respond {
                    let enable = server.enable_encryption(key, token);
                    if let Ok(_) = enable {
                        let read = self.read_packet().await;
                        if let Ok(packet) = read {
                            match packet {
                                Packet::LoginSetCompression(body) => {
                                    self.set_compression_threshold(body.threshold.0).await
                                }
                                Packet::LoginSuccess(_) => {
                                    self.connected = true;
                                    self.set_state(State::Play)
                                }
                                _ => Err(anyhow::anyhow!(WRONG_PACKET_ERROR)),
                            }
                        } else {
                            Err(read.err().unwrap())
                        }
                    } else {
                        enable
                    }
                } else {
                    respond
                }
            } else {
                auth
            }
        } else {
            Err(anyhow::anyhow!(SERVER_NONE_ERROR))
        }
    }

    /// Completes the clientside part of the Minecraft login sequence.
    async fn login(&mut self) -> Result<()> {
        if self.connected {
            return Err(anyhow::anyhow!("Already connected."));
        }
        let read = self.read_packet().await;
        if let Ok(packet) = read {
            match packet {
                Packet::LoginEncryptionRequest(body) => self.enable_encryption(body).await,
                Packet::LoginSetCompression(body) => {
                    self.set_compression_threshold(body.threshold.0).await
                }
                Packet::LoginSuccess(_) => {
                    self.connected = true;
                    self.set_state(State::Play)
                }
                _ => Err(anyhow::anyhow!(WRONG_PACKET_ERROR)),
            }
        } else {
            Err(read.err().unwrap())
        }
    }
}
