use super::{connection::MinecraftConnection, proto, Packet};
use anyhow::Result;
use mcproto_rs::{protocol::State, types::CountedArray};
use std::net::SocketAddr;

use std::sync::Arc;
use tokio::{sync::{
    mpsc::{self, error::TryRecvError, Receiver, Sender},
    Mutex,
}, runtime::Runtime, task::JoinHandle, join};

use super::server::PacketListener;

/// Error text when the client is not connected to a server.
const SERVER_NONE_ERROR: &str = "Not connected to server.";
/// Error when the client recieves a Packet it was not expecting.
const WRONG_PACKET_ERROR: &str = "Recieved an unexpected packet.";

/// A Minecraft client, a wrapper for the utilities required to connect to a Minecraft server and perform various activities in said server.
pub struct MinecraftClient {
    /// The socket address of the server the client will connect to.
    address: SocketAddr,
    /// Tokio runtime
    runtime: Runtime,
    /// Running instance of the client
    runner: Arc<Mutex<ClientRunner>>,
    packet_listeners: Arc<Mutex<Vec<PacketListener>>>,
}

struct ClientRunner {
    /// The game profile that will be used to authenticate with the server.
    profile: crate::auth::Profile,
    /// Whether or not the client is currently connected to the server.
    connected: bool,
    /// When the client is connected, this holds the actual connection to the server.
    server: Option<MinecraftConnection>,
    packet_send: Option<mpsc::Sender<Packet>>,
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
    /// use rust_mc::minecraft::client::MinecraftClient;
    /// use rust_mc::mojang::auth::Profile;
    /// use std::net::{IpAddr, Ipv4Addr, SocketAddr};
    ///
    /// let mut client = MinecraftClient::new(
    ///     "127.0.0.1:25565".parse().unwrap(),
    ///     Profile::new("rust_mc", "", true),
    /// );
    /// ```
    pub fn new(address: SocketAddr, profile: crate::auth::Profile) -> MinecraftClient {
        let runner = Arc::new(Mutex::new(
            ClientRunner {
                profile,
                server: None,
                connected: false,
                packet_send: None,
            }
        ));
        Self {
            address,
            runtime: Runtime::new().unwrap(),
            runner,
            packet_listeners: Arc::new(Mutex::new(vec![])),
        }
    }

    /// Tells the client to connect to the Minecraft server at the socket address in it's `address` field.
    ///
    /// # Examples
    ///
    /// This example requires a Minecraft server to be running on localhost:25565.
    ///
    /// ```rust
    /// use rust_mc::minecraft::client::MinecraftClient;
    /// use rust_mc::mojang::auth::Profile;
    /// use std::net::{IpAddr, Ipv4Addr, SocketAddr};
    /// use std::sync::Arc;
    /// use tokio::sync::Mutex;
    /// use futures::executor::block_on;
    ///
    /// let mut client = MinecraftClient::new(
    ///     "127.0.0.1:25565".parse().unwrap(),
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
        self.runtime.handle().block_on(fut)?;

        // Create channels for communicating to the client
        let (tx, rx) = mpsc::channel(20);

        let (send_packet, receive_packet) = mpsc::channel(1024);
        {
            self.runner.lock().await.packet_send = Some(send_packet);
        }

        let packet_listeners = self.packet_listeners.clone();
        let loop_runner_mut = self.runner.clone();

        let main_client_loop = async move {
            ClientRunner::start_loop(loop_runner_mut, rx).await;
        };
        let packet_listen_loop = async move {
            let mut recv = receive_packet;
            let mut pls = packet_listeners;
            loop {
                MinecraftClient::receive_packet(&mut pls, &mut recv).await;
            }
        };

        // Start the client loop in a new thread
        let handle = self.runtime.spawn(async move {
            // Use a single thread to process receiving packets and listening here for packets
            join!(main_client_loop, packet_listen_loop);
        });
        
        Ok((handle, tx))
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
    /// use rust_mc::minecraft::client::MinecraftClient;
    /// use rust_mc::mojang::auth::Profile;
    /// use std::net::{IpAddr, Ipv4Addr, SocketAddr};
    /// use futures::executor::block_on;
    ///
    /// let mut client = MinecraftClient::new(
    ///     "127.0.0.1:25565".parse().unwrap(),
    ///     Profile::new("rust_mc", "", true),
    /// );
    ///
    /// block_on(client.connect()); // Connect the client before sending the message.
    ///
    /// block_on(client.send_chat_message("Hello!")); // Send the chat message. Assumes success of connection
    /// ```
    pub async fn send_chat_message(&self, msg: &str) {
        let mut runner = self.runner.lock().await;
        runner.send_chat_message(msg).await.unwrap();
    }

    /// Register a packet listener to intercept packets received by the server
    pub async fn on_receive_packet(&self, cb: PacketListener) {
        self.packet_listeners.lock().await.push(cb);
    }

    /// Attempts to receive a packet from the ClientRunner and calls the packet listeners
    async fn receive_packet(packet_listeners: &mut Arc<Mutex<Vec<PacketListener>>>, recv: &mut mpsc::Receiver<Packet>) {
        if let Some(packet) = recv.recv().await {
            let pl_lock = packet_listeners.lock().await;
            pl_lock.iter().for_each(|f| {
                f(&packet);
            })
        }
    }
}

impl ClientRunner {
    /// Starts the client loop and listens for incoming server packets
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
                    // Attempt to read a client packet. Only attempt for 5 ms to allow others to take the lock
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
                // Process a packet if deserialized correctly
                match packet_read {
                    Ok(packet) => client_lock.handle_packet(packet).await,
                    Err(e) => {
                        error!("[Client] {e}")
                    },
                }
            }
        }
    }

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
        match &packet {
            Packet::PlayServerChatMessage(body) => {
                if body.sender != self.profile.game_profile.id {
                    if let Some(message) = body.message.to_traditional() {
                        debug!("[Client] {}", message);
                    } else {
                        debug!("[Client] Raw message: {:?}", body);
                    }
                }
            }
            _ => {}
        };

        if let Some(sender) = &mut self.packet_send {
            sender.send(packet).await.unwrap();
        }
    }
}
