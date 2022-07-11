use std::{
    collections::{HashMap, HashSet},
    convert::TryInto,
    sync::Arc, net::SocketAddr,
};

use mcproto_rs::status::StatusPlayerSampleSpec;

use tokio::{sync::{mpsc, Mutex, MutexGuard}, net::TcpStream, task::JoinHandle};
use tokio::{net::TcpListener, runtime::Runtime};

use super::{connection::MinecraftConnection, proto, status::ServerStatus, types::Player, Packet};
use anyhow::Result;
use mcproto_rs::{types::Chat, uuid::UUID4};
use proto::ChatPosition;

use openssl::rsa::{Rsa, Padding};

type NameUUID = (String, UUID4);
type ConnectedPlayers = Arc<Mutex<HashMap<Arc<NameUUID>, Arc<Mutex<ServerClient>>>>>;
/// Callback function that acts on a packet
type PacketListener = fn(&Packet) -> ();

/// Represents a Minecraft server.
pub struct MinecraftServer {
    /// Address to bind the listener to.
    bind_address: String,
    /// Tokio runtime. Required for creating threads and having compatibility with mctokio crate
    runtime: Arc<Mutex<Runtime>>,
    // MC Runner
    runner: Arc<Mutex<ServerRunner>>,
    /// List of packet listeners
    packet_listeners: Arc<Mutex<Vec<PacketListener>>>,
}

struct ServerRunner {
    /// Clients that have connected.
    players: ConnectedPlayers,
    /// Whether or not to make sure players have authenticated with Mojang, also enables packet encryption.
    online: bool,
    /// The server's status.
    status: ServerStatus,
    /// Entity IDs in use.
    used_entity_ids: HashSet<i32>,
    /// Whether or not players should be banned on death
    hardcore: bool,
    /// Receiving channel for server communication
    receiver: mpsc::Receiver<()>,
    /// Whether the server is running
    running: bool,
    /// Sends packets to the MinecraftServer
    packet_send: Option<mpsc::Sender<Packet>>,
}

impl MinecraftServer {
    /// Create a new server that binds to an address, does not launch/start the server.
    ///
    /// # Arguments
    ///
    /// * `bind_address` local address to bind to and listen for connections on
    /// * `online` whether or not to check if players have authenticated with Mojang before letting them join, also enalbes encryption
    ///
    /// # Examples
    ///
    /// ```rust
    /// use rust_mc::minecraft::server::Server;
    ///
    /// let server = Server::new("127.0.0.1:25565", false);
    /// ```
    pub fn new(
        bind_address: String,
        description: String,
        max_players: i32,
        online: bool
    ) -> (Self, mpsc::Sender<()>) {
        use mcproto_rs::status::{StatusPlayersSpec, StatusVersionSpec};
        let status = ServerStatus {
            description: Chat::from_text(&description),
            players: StatusPlayersSpec {
                max: max_players,
                online: 0,
                sample: vec![],
            },
            version: StatusVersionSpec {
                name: "rust-mc 1.16.3".to_string(),
                protocol: 753,
            },
            favicon: None,
        };

        let runtime = Arc::new(Mutex::new(
            Runtime::new().unwrap()
        ));
        
        // Create channel to allow for server shutdown in another thread
        let (tx, rx) = mpsc::channel(20);

        let runner = Arc::new(Mutex::new(
            ServerRunner {
                players: Arc::new(Mutex::new(HashMap::new())),
                status,
                online,
                used_entity_ids: HashSet::new(),
                hardcore: false,
                receiver: rx,
                running: false,
                packet_send: None,
            }
        ));

        (
            MinecraftServer {
                bind_address,
                runtime,
                runner,
                packet_listeners: Arc::new(Mutex::new(vec![])),
            },
            tx,
        )
    }

    /// Starts listening for tcp connections on `self.bind_address`.
    /// Fully handles client connections, performs the login sequence and starts a new thread listening for packets from that client.
    ///
    /// # Arguments
    ///
    /// * `self_mutex` An Arc-Mutex containing the server that should start listening, mutex is locked as little as possible.
    /// * `receiver` Receiver part of an rx/tx channel, sending anything or closing the channel causes this to stop looping.
    /// * `runtime` Arc-Mutex containing a tokio runtime, locked as little as possible, used for launching threads to handle individual clients.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use rust_mc::minecraft::server::Server;
    /// use std::sync::Arc;
    /// use tokio::{
    ///     runtime::Runtime,
    ///     sync::{mpsc, Mutex}
    /// };
    ///
    /// let server = Arc::new(Mutex::new(Server::new("127.0.0.1:25565", false)));
    /// let (tx, rx) = mpsc::channel(20);
    /// let runtime = Arc::new(Mutex::new(Runtime::new().unwrap()));
    ///
    /// Server::start(server, rx, runtime); // Start the server.
    ///
    /// std::thread::delay(std::time::Duration::from_millis(1000));
    /// tx.try_lock().send(()); // Stop the server after 1000ms by sending something.
    /// ```
    pub async fn start(&self) -> Result<JoinHandle<()>> {
        let runner_mut = self.runner.clone();

        let runtime_server_loop = self.runtime.clone();

        let loop_runner_mut = runner_mut.clone();

        // Set up a channel to communicate error or success
        let (mut tx, mut rx) = mpsc::channel(32);

        let bind_address = self.bind_address.clone();
        
        // Receives packets from the ServerRunner
        let (packet_send, packet_receive) = mpsc::channel(1024);
        {
            self.runner.lock().await.packet_send = Some(packet_send);
        }

        let packet_listeners = self.packet_listeners.clone();

        // In a new thread, loop while the server is running
        let server_loop = async move {
            let mut listener: TcpListener;
            let connections: ConnectedPlayers;
            {
                // Bind the socket
                let self_lock = runner_mut.lock().await;
                let bind = TcpListener::bind(bind_address.clone()).await;
                if let Ok(bind) = bind {
                    listener = bind;
                    connections = self_lock.players.clone();
                    tx.send(Ok(())).await.unwrap();
                } else {
                    println!("{}", bind.err().unwrap());
                    tx.send(Err(anyhow::anyhow!(
                        "Failed to bind to {}",
                        bind_address.clone()
                    ))).await.unwrap();
                    return;
                }
            }
            println!("Started server on {bind_address}");
            // Set server to running
            {
                let mut self_lock = loop_runner_mut.lock().await;
                self_lock.running = true;
            }
            // Main server loop that listens for incoming clients
            let main_server_loop = async {
                loop {
                    {
                        let mut self_lock = loop_runner_mut.lock().await;
                        let receiver = &mut self_lock.receiver;
                        // Shut the server down if we close the channel
                        if let Err(err) = receiver.try_recv() {
                            if err == tokio::sync::mpsc::error::TryRecvError::Closed {
                                break;
                            }
                        }
                    }
                    // Listen for incoming clients
                    if let Ok((socket, address)) = listener.accept().await {
                        let self_join_arc = loop_runner_mut.clone();
                        let connections = connections.clone();
                        let runtime_arc = runtime_server_loop.clone();
                        let join = async move {
                            ServerRunner::handle_client_connect(self_join_arc, runtime_arc, socket, address, connections).await;
                        };
                        runtime_server_loop.lock().await.spawn(join);
                    }
                }
            };
            // Async block to receive packets from the ServerRunner
            let packet_listen_loop = async move {
                let mut recv = packet_receive;
                let mut pls = packet_listeners;
                loop {
                    MinecraftServer::receive_packet(&mut pls, &mut recv).await;
                }
            };

            // Run the main server loop and packet listening loop in the same thread
            // Since both loops spend most of their time blocked, this is better than spawning 2 threads
            // They both will halt once the server dies
            tokio::join!(main_server_loop, packet_listen_loop);

            // Set server to not running
            {
                let mut self_lock = loop_runner_mut.lock().await;
                self_lock.running = false;
            }
        };

        // Spawn server thead
        let handle = self.runtime.lock().await.spawn(server_loop);
        let err = rx.recv().await;
        match err {
            Some(e) => {
                if let Err(e) = e {
                    Err(e)
                } else {
                    Ok(handle)
                }
            },
            None => Err(anyhow::anyhow!(
                "Unexpected channel shutdown"
            ))
        }
    }

    pub async fn stop(&mut self) {
        let mut runner = self.runner.lock().await;
        if runner.running {
            runner.receiver.close();
        }
    }

    pub async fn get_connected_players(&self) -> Vec<StatusPlayerSampleSpec> {
        let mut runner = self.runner.lock().await;
        runner.get_connected_players().await
    }

    /// Register a packet listener to intercept packets received by the server
    pub async fn on_receive_packet(&self, cb: PacketListener) {
        self.packet_listeners.lock().await.push(cb);
    }

    /// Attempts to receive a packet from the ServerRunner and calls the packet listeners
    async fn receive_packet(packet_listeners: &mut Arc<Mutex<Vec<PacketListener>>>, recv: &mut mpsc::Receiver<Packet>) {
        if let Some(packet) = recv.recv().await {
            let pl_lock = packet_listeners.lock().await;
            pl_lock.iter().for_each(|f| {
                f(&packet);
            })
        }
    }
}

impl ServerRunner {
    async fn handle_client_connect(self_join_arc: Arc<Mutex<ServerRunner>>, runtime_arc: Arc<Mutex<Runtime>>, socket: TcpStream, address: SocketAddr, connections: ConnectedPlayers) {
        let mut client = MinecraftConnection::from_tcp_stream(socket);
        let handshake = client.handshake(None, None).await;
        if let Ok(result) = handshake {
            println!(
                "{} handshake with {} successful.",
                result.name(),
                address.to_string()
            );
            if result == mcproto_rs::protocol::State::Login {
                let login;
                {
                    let mut self_lock = self_join_arc.lock().await;
                    if connections.clone().lock().await.len()
                        >= self_lock.status.players.max.try_into().unwrap()
                    {
                        let _kick = Self::login_kick(
                            client,
                            Chat::from_text(
                                "Server is full, wait for another player to leave.",
                            ),
                        )
                        .await;
                        return;
                    }
                    login = self_lock.handle_login(&mut client, 256).await;
                }
                if let Ok(login) = login {
                    let mut entity_id = i32::MIN;
                    {
                        let mut self_lock = self_join_arc.lock().await;
                        while self_lock.used_entity_ids.contains(&entity_id) {
                            entity_id = entity_id + 1;
                        }
                        self_lock.used_entity_ids.insert(entity_id);
                    }
                    let server_client = Arc::new(Mutex::new(ServerClient {
                        name: login.0.clone(),
                        uuid: login.1.clone(),
                        entity_id,
                        player: Player::new(
                            login.0.clone(),
                            login.1.clone(),
                            entity_id,
                        ),
                        connection: client,
                        view_distance: 10
                    }));
                    {
                        for player in connections.lock().await.keys() {
                            if player.0 == login.0 || player.1 == login.1 {
                                let _kick = server_client.lock().await.kick(Chat::from_text("Someone with the same name or UUID as you is already connected.")).await;
                                return;
                            }
                        }
                    }
                    let server_client_arc = server_client.clone();
                    let self_loop_arc = self_join_arc.clone();
                    let packet_loop = async move {
                        let client_arc = server_client_arc.clone();
                        let server_arc = self_loop_arc.clone();
                        // Loop for processing player packets
                        loop {
                            let packet_read: Result<Option<super::Packet>>;
                            {
                                packet_read = client_arc
                                    .lock()
                                    .await
                                    .connection
                                    .read_next_packet()
                                    .await;
                            }
                            if let Ok(packet_ok) = packet_read {
                                if let Some(packet) = packet_ok {
                                    server_arc
                                        .lock()
                                        .await
                                        .handle_packet(
                                            packet,
                                            server_client_arc.clone().lock().await,
                                        )
                                        .await;
                                }
                            };
                        }
                    };
                    // Send JoinGame packet to player
                    {
                        let self_lock = self_join_arc.lock().await;
                        if let Err(e) = server_client.lock().await.join(self_lock.hardcore, self_lock.status.players.max).await {
                            eprintln!("{:?}", e);
                            return;
                        }
                    }
                    runtime_arc.lock().await.spawn(packet_loop);
                    connections
                        .lock()
                        .await
                        .insert(Arc::new((login.0, login.1)), server_client);
                    println!("{} successfully logged in.", address.to_string());
                } else {
                    println!(
                        "{} failed to log in: {}",
                        address.to_string(),
                        login.err().unwrap()
                    )
                }
            } else {
                let status: Result<()>;
                {
                    status = self_join_arc.lock().await.handle_status(client).await;
                }
                if let Ok(_) = status {
                    println!(
                        "{} successfully got server status.",
                        address.to_string()
                    )
                } else {
                    println!("{} failed to get server status.", address.to_string())
                }
            }
        } else {
            println!(
                "Handshake with {} failed: {}",
                address.to_string(),
                handshake.err().unwrap()
            )
        }
    }

    /// Handle client login.
    async fn handle_login(
        &mut self,
        client: &mut MinecraftConnection,
        compression_threshold: i32,
    ) -> Result<(String, UUID4)> {
        use super::Packet::{
            LoginEncryptionRequest, LoginEncryptionResponse, LoginSetCompression, LoginStart,
            LoginSuccess,
        };
        use crate::mojang::auth;
        use mcproto_rs::protocol::State::Play;
        use mcproto_rs::types::CountedArray;
        use proto::{LoginEncryptionRequestSpec, LoginSetCompressionSpec, LoginSuccessSpec};
        let second = &mut client.read_next_packet().await;
        if let Ok(Some(LoginStart(body))) = second {
            let response_spec = LoginSetCompressionSpec {
                threshold: mcproto_rs::types::VarInt::from(compression_threshold),
            };
            let mut result = (body.name.clone(), UUID4::random());
            if self.online {
                let server_id = "".to_string();
                let rsa = Rsa::generate(1024).unwrap();
                let public_key = rsa.public_key_to_der().unwrap();
                let verify_token: &mut [u8] = &mut [0; 16];
                for mut _i in verify_token.iter_mut() {
                    *_i = rand::random::<u8>();
                }
                let encryption_spec = LoginEncryptionRequestSpec {
                    server_id: server_id.clone(),
                    public_key: CountedArray::from(public_key.clone()),
                    verify_token: CountedArray::from(verify_token.to_vec()),
                };
                if let Err(error) = client
                    .write_packet(LoginEncryptionRequest(encryption_spec))
                    .await
                {
                    return Err(error);
                } else {
                    let response = client.read_next_packet().await;
                    if let Ok(Some(LoginEncryptionResponse(response))) = response {
                        // Decrypt the verify token from the client
                        let mut decrypted_token: Vec<u8> = vec![0; rsa.size() as usize];
                        rsa.private_decrypt(&response.verify_token, &mut decrypted_token, Padding::PKCS1).unwrap();

                        // Decrypted the shared secret
                        let mut shared_secret: Vec<u8> = vec![0; rsa.size() as usize];
                        rsa.private_decrypt(&response.shared_secret, &mut shared_secret, Padding::PKCS1).unwrap();
                        let shared_secret = &shared_secret[..16];

                        // Compare decrypted token with the original verify token
                        if decrypted_token[..16] == verify_token.to_vec() {
                            let verify = auth::verify_join(
                                &body.name,
                                server_id,
                                &shared_secret,
                                &public_key,
                            )
                            .await;
                            if let Ok(verified) = verify {
                                if let Err(error) =
                                    client.enable_encryption(&shared_secret, &shared_secret)
                                {
                                    return Err(error);
                                }
                                result = verified;
                            } else {
                                return Err(verify.err().unwrap());
                            }
                        } else {
                            return Err(anyhow::anyhow!(
                                "Client did not send the correct response to encryption request. {:?}", response
                            ));
                        }
                    } else {
                        println!("{:?}", response);
                        if let Some(error) = response.err() {
                            return Err(error);
                        } else {
                            return Err(anyhow::anyhow!(
                                "Client did not send a valid response to the encryption request."
                            ));
                        }
                    }
                }
            };
            if let Err(error) = client
                .write_packet(LoginSetCompression(response_spec))
                .await
            {
                return Err(error);
            } else {
                client.set_compression_threshold(compression_threshold);
            }
            if let Err(error) = client
                .write_packet(LoginSuccess(LoginSuccessSpec {
                    username: result.0.clone(),
                    uuid: result.1.clone(),
                }))
                .await
            {
                return Err(error);
            }
            client.set_state(Play);
            return Ok(result);
        } else {
            return Err(anyhow::anyhow!(
                "Client did not follow up with login start."
            ));
        }
    }

    /// Handle status requests by a client.
    async fn handle_status(&mut self, mut client: MinecraftConnection) -> anyhow::Result<()> {
        use super::Packet::{StatusPing, StatusPong, StatusRequest};
        use mcproto_rs::status::StatusPlayerSampleSpec;
        use proto::StatusPongSpec;
        let second = &mut client.read_next_packet().await;
        if let Ok(second) = second {
            if let Some(StatusRequest(_)) = second {
                {
                    let connected_players = self.players.lock().await;
                    self.status.players.online = connected_players.len().try_into().unwrap();
                    let mut players: Vec<StatusPlayerSampleSpec> = vec![];
                    for player in connected_players.keys() {
                        players.push(StatusPlayerSampleSpec {
                            id: player.1,
                            name: player.0.clone(),
                        });
                    }
                    self.status.players.sample = players;
                }
                if let Err(error) = self.status.send_status(&mut client).await {
                    return Err(error);
                }
                let third = client.read_next_packet().await;
                if let Ok(third) = third {
                    if let Some(StatusPing(body)) = third {
                        if let Err(error) = client
                            .write_packet(StatusPong(StatusPongSpec {
                                payload: body.payload,
                            }))
                            .await
                        {
                            return Err(error);
                        }
                    }
                }
                return Ok(());
            } else {
                return Err(anyhow::anyhow!(
                    "Client did not send valid packet after login handshake."
                ));
            }
        } else {
            return Err(anyhow::anyhow!(
                "Client did not send valid packet after login handshake."
            ));
        }
    }

    /// Kicks a player that is in the login state.
    ///
    /// # Arguments
    ///
    /// * `connection` Connection of the player to kick.
    /// * `message` Message to send the player (reason).
    pub async fn login_kick(mut connection: MinecraftConnection, message: Chat) -> Result<()> {
        use proto::LoginDisconnectSpec;
        use Packet::LoginDisconnect;
        let spec = LoginDisconnectSpec { message };
        connection.write_packet(LoginDisconnect(spec)).await
    }

    /// Send a chat message to all connected clients.
    ///
    /// # Arguments
    ///
    /// * `message` Chat message to send.
    #[allow(unused_must_use)]
    pub async fn broadcast_chat(&mut self, message: Chat) {
        for player in self.players.clone().lock().await.values() {
            player.clone().lock().await.send_message(
                message.clone(),
                ChatPosition::ChatBox,
                None,
            ).await;
        }
    }

    /// Handle packets sent by connected clients.
    async fn handle_packet(&mut self, packet: Packet, sender: MutexGuard<'_, ServerClient>) {
        println!("Received a packet {:?}", packet);

        match &packet {
            Packet::PlayClientChatMessage(body) => {
                // Create chat message
                let chat_message = &("<".to_owned() + sender.name.as_str() + "> " + body.message.as_str());
                // Release the lock on the ServerClient since broadcast_chat needs the lock
                drop(sender);
                // Broadcast the message to the whole server
                self.broadcast_chat(Chat::from_traditional(
                    chat_message,
                    true,
                ))
                .await;
            }
            _ => {
            }
        }

        // Send packet away
        if let Some(sender) = &mut self.packet_send {
            sender.send(packet).await.unwrap();
        }
    }

    async fn get_connected_players(&mut self) -> Vec<StatusPlayerSampleSpec> {
        let connected_players = self.players.lock().await;
        connected_players.keys()
            .map(|player| StatusPlayerSampleSpec {
                id: player.1,
                name: player.0.clone(),
            })
            .collect()
    }
}

/// Represents a connected client/player.
struct ServerClient {
    name: String,
    uuid: UUID4,
    entity_id: i32,
    player: Player,
    view_distance: i32,
    connection: MinecraftConnection,
}

impl ServerClient {
    /// Sends the client a message to be displayed in the chosen receiving area.
    ///
    /// # Arguments
    ///
    /// * `message` Message to send (using the Chat format)
    /// * `position` what receiving area the message should be displayed in
    /// * `sender` Who sent the message, if none is given then UUID 0 is used.
    pub async fn send_message(
        &mut self,
        message: Chat,
        position: ChatPosition,
        sender: Option<UUID4>,
    ) -> Result<()> {
        use super::Packet::PlayServerChatMessage;
        use proto::PlayServerChatMessageSpec;
        let spec = PlayServerChatMessageSpec {
            message,
            sender: sender.unwrap_or(UUID4::from(0)),
            position,
        };
        let packet = PlayServerChatMessage(spec);
        // println!("[Server] Packet: {:?}", packet);
        self.connection.write_packet(packet).await
    }

    /// Kicks the client.
    ///
    /// # Arguments
    ///
    /// * `reason` Reason for the kick.
    pub async fn kick(&mut self, reason: Chat) -> Result<()> {
        use proto::PlayDisconnectSpec;
        use Packet::PlayDisconnect;
        let spec = PlayDisconnectSpec { reason };
        self.connection.write_packet(PlayDisconnect(spec)).await
    }

    pub async fn join(&mut self, is_hardcore: bool, max_players: i32) -> Result<()> {
        let spec = proto::PlayJoinGameSpec {
            gamemode: self.player.gamemode.clone(),
            previous_gamemode: proto::PreviousGameMode::Previous(self.player.gamemode.clone()),
            entity_id: self.player.entity_id,
            is_hardcore,
            worlds: mcproto_rs::types::CountedArray::from(vec![String::from("world")]),
            dimension_codec: mcproto_rs::types::NamedNbtTag { root: mcproto_rs::nbt::NamedTag { name: String::from("minecraft:overworld"), payload: mcproto_rs::nbt::Tag::End} },
            dimension: mcproto_rs::types::NamedNbtTag { root: mcproto_rs::nbt::NamedTag { name: String::from("minecraft:overworld"), payload: mcproto_rs::nbt::Tag::End} },
            world_name: String::from("world"),
            hashed_seed: 0,
            max_players: max_players.try_into().unwrap_or_default(),
            view_distance: mcproto_rs::types::VarInt::from(self.view_distance),
            enable_respawn_screen: false,
            is_flat: false,
            is_debug: false,
            reduced_debug_info: true,
        };
        println!("Sent join packet");
        self.connection.write_packet(Packet::PlayJoinGame(spec)).await
    }
}
