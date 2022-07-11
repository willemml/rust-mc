use super::{connection::MinecraftConnection, proto, Packet};
use anyhow::Result;
use futures::pin_mut;
use mcproto_rs::{protocol::State, types::CountedArray};
use std::net::SocketAddr;

use std::sync::Arc;
use tokio::{sync::{
    mpsc::{self, error::TryRecvError, Receiver, Sender},
    Mutex,
}, runtime::Runtime, task::JoinHandle};

/// Error text when the client is not connected to a server.
const SERVER_NONE_ERROR: &str = "Not connected to server.";
/// Error when the client recieves a Packet it was not expecting.
const WRONG_PACKET_ERROR: &str = "Recieved an unexpected packet.";

/// A Minecraft client, a wrapper for the utilities required to connect to a Minecraft server and perform various activities in said server.
pub struct MinecraftClient {
    /// The socket address of the server the client will connect to.
    address: SocketAddr,
    /// Tokio runtime
    runtime: Arc<Mutex<Runtime>>,
    /// Running instance of the client
    runner: Arc<Mutex<ClientRunner>>,
}

struct ClientRunner {
    /// The game profile that will be used to authenticate with the server.
    profile: crate::auth::Profile,
    /// Whether or not the client is currently connected to the server.
    connected: bool,
    /// When the client is connect this holds the actual connection to the server.
    server: Option<MinecraftConnection>,
}

impl MinecraftClient {
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
    pub fn new(address: SocketAddr, profile: crate::auth::Profile) -> MinecraftClient {
        let runner = Arc::new(Mutex::new(
            ClientRunner {
                profile,
                server: None,
                connected: false,
            }
        ));
        Self {
            address,
            runtime: Arc::new(Mutex::new(Runtime::new().unwrap())),
            runner,
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
    pub async fn connect(&mut self) -> Result<(JoinHandle<()>, Sender<()>)> {
        let runner_mut = self.runner.clone();

        let fut = async {
            let mut runner = runner_mut.lock().await;
            // Authenticate profile
            let auth = runner.profile.authenticate().await;
    
            // Check if the auth succeeded
            if let Err(err) = auth {
                return Err(err);
            }    
            // Create a connection that can be used to connect to the server
            let connection = MinecraftConnection::connect_async(self.address).await?;
    
            // Save our connection
            runner.server = Some(connection);
            
            let profile = runner.profile.game_profile.name.clone();

            // Perform a full handshake with the server
            let server = runner.server.as_mut().unwrap();
            server.handshake(
                Some(proto::HandshakeNextState::Login),
                Some(profile),
            ).await?;
            // Do the login procedure
            runner.login().await
        };
        // Start the TCP connection in the tokio runtime. Check for errors
        let mut runtime = self.runtime.lock().await;
        runtime.block_on(fut)?;

        // Create channels for communicating to the client
        let (tx, rx) = mpsc::channel(20);

        // Start the client loop in a new thread
        let loop_runner_mut = self.runner.clone();
        let handle = runtime.spawn(async move {
            ClientRunner::start_loop(loop_runner_mut, rx).await;
        });
        
        Ok((handle, tx))
    }

    pub async fn send_chat_message(&self, msg: &str) {
        let mut runner = self.runner.lock().await;
        runner.send_chat_message(msg).await.unwrap();
    }
}

impl ClientRunner {
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
    async fn start_loop(client: Arc<Mutex<Self>>, mut receiver: Receiver<()>) {
        let client_arc = client.clone();
        loop {
            if let Err(err) = receiver.try_recv() {
                if err == TryRecvError::Closed {
                    println!("[Client] Closed receiver connection");
                    break;
                }
                let mut client_lock = client_arc.lock().await;
                if !client_lock.connected {
                    println!("[Client] Disconnected from server");
                    break;
                }
                let packet_read: Result<super::Packet>;
                {
                    let delayer = client_lock.read_packet();
                    if let Ok(packet) =
                        tokio::time::timeout(std::time::Duration::from_millis(5), delayer)
                            .await
                    {
                        packet_read = packet;
                    } else {
                        continue;
                    }
                }
                match packet_read {
                    Ok(packet) => client_lock.handle_packet(packet).await,
                    Err(e) => {
                        println!("[Client] {e}")
                    },
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
    async fn send_chat_message(&mut self, message: &str) -> Result<()> {
        let spec = proto::PlayClientChatMessageSpec {
            message: message.to_string(),
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
                        Packet::LoginSuccess(spec) => self.login_success(spec),
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
            let auth = self.profile.join_server(spec.server_id, buf, key).await;
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
                                Packet::LoginSuccess(spec) => self.login_success(spec),
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

    /// Executed when server login succeeds.
    fn login_success(&mut self, spec: proto::LoginSuccessSpec) -> Result<()> {
        self.connected = true;
        if self.profile.offline {
            self.profile.game_profile.id = spec.uuid;
            self.profile.game_profile.name = spec.username;
        }
        self.set_state(State::Play)
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
                Packet::LoginSuccess(spec) => self.login_success(spec),
                _ => Err(anyhow::anyhow!(WRONG_PACKET_ERROR)),
            }
        } else {
            Err(read.err().unwrap())
        }
    }

    /// Handle received packets.
    async fn handle_packet(&mut self, packet: Packet) {
        match packet {
            Packet::PlayServerChatMessage(body) => {
                if body.sender != self.profile.game_profile.id {
                    if let Some(message) = body.message.to_traditional() {
                        println!("[Client] {}", message);
                    } else {
                        println!("Raw message: {:?}", body);
                    }
                }
            }
            _ => {}
        };
    }
}
