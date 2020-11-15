use std::{collections::HashMap, convert::TryInto, sync::Arc};

use tokio::{net::TcpListener, runtime::Runtime};
use tokio::sync::{MutexGuard, Mutex, mpsc::Receiver};

use super::{connection::MinecraftConnection, proto, status::ServerStatus, Packet};
use anyhow::Result;
use mcproto_rs::{types::Chat, uuid::UUID4};
use proto::ChatPosition;

type NameUUID = (String, UUID4);
type ConnectedPlayers = Arc<Mutex<HashMap<Arc<NameUUID>, Arc<ServerClient>>>>;

/// Represents a Minecraft server.
pub struct Server {
    /// Clients that have connected.
    players: ConnectedPlayers,
    /// Address to bind the listener to.
    bind_address: String,
    /// Whether or not to make sure players have authenticated with Mojang, also enables packet encryption.
    online: bool,
    /// The server's status.
    status: ServerStatus,
}

impl Server {
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
    pub fn new(bind_address: String, description: String, max_players: i32, online: bool) -> Self {
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
        Server {
            players: Arc::new(Mutex::new(HashMap::new())),
            bind_address,
            status,
            online,
        }
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
    pub async fn start(
        self_mutex: Arc<Mutex<Self>>,
        mut receiver: Receiver<()>,
        runtime: Arc<Mutex<Runtime>>,
    ) -> Result<()> {
        let mut listener: TcpListener;
        let connections: ConnectedPlayers;
        {
            let self_lock = self_mutex.lock().await;
            let bind = TcpListener::bind(self_lock.bind_address.clone()).await;
            if let Ok(bind) = bind {
                listener = bind;
                connections = self_lock.players.clone();
            } else {
                println!("{}", bind.err().unwrap());
                return Err(anyhow::anyhow!(
                    "Failed to bind to {}",
                    self_lock.bind_address.clone()
                ));
            }
        }
        loop {
            if let Err(err) = receiver.try_recv() {
                if err == tokio::sync::mpsc::error::TryRecvError::Closed {
                    break;
                }
                if let Ok((socket, address)) = listener.accept().await {
                    let self_join_arc = self_mutex.clone();
                    let connections = connections.clone();
                    let runtime_arc = runtime.clone();
                    let join = async move {
                        let mut client = MinecraftConnection::from_tcp_stream(socket);
                        let handshake = client.handshake(None, None).await;
                        let client_arc = Arc::new(Mutex::new(client));
                        if let Ok(result) = handshake {
                            println!(
                                "{} handshake with {} successful.",
                                result.name(),
                                address.to_string()
                            );
                            if result == mcproto_rs::protocol::State::Login {
                                let login: Result<(String, UUID4)>;
                                {
                                    let mut self_lock = self_join_arc.lock().await;
                                    if connections.clone().lock().await.len()
                                        >= self_lock.status.players.max.try_into().unwrap()
                                    {
                                        let _kick = Self::login_kick(client_arc.lock().await, Chat::from_text("Server is full, wait for another player to leave.")).await;
                                        return;
                                    }
                                    login = self_lock.handle_login(client_arc.clone(), 256).await;
                                }
                                if let Ok(login) = login {
                                    let server_client = Arc::new(ServerClient {
                                        name: login.0,
                                        uuid: login.1,
                                        connection: client_arc.clone(),
                                    });
                                    let server_client_arc = server_client.clone();
                                    let self_loop_arc = self_join_arc.clone();
                                    let client_loop_arc = client_arc.clone();
                                    let packet_loop = async move {
                                        let client_arc = client_loop_arc.clone();
                                        let server_arc = self_loop_arc.clone();
                                        loop {
                                            let packet_read: Result<Option<super::Packet>>;
                                            {
                                                packet_read = client_arc
                                                    .lock()
                                                    .await
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
                                                            server_client_arc.clone(),
                                                        )
                                                        .await;
                                                }
                                            };
                                        }
                                    };
                                    runtime_arc.lock().await.spawn(packet_loop);
                                    connections.lock().await.insert(
                                        Arc::new((
                                            server_client.name.clone(),
                                            server_client.uuid.clone(),
                                        )),
                                        server_client,
                                    );
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
                                    status = self_join_arc
                                        .lock()
                                        .await
                                        .handle_status(client_arc.clone())
                                        .await;
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
                    };
                    runtime.lock().await.spawn(join);
                }
            }
        }
        return Ok(());
    }

    /// Handle client login.
    async fn handle_login(
        &mut self,
        client_mutex: Arc<Mutex<MinecraftConnection>>,
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
        let client = &mut client_mutex.lock().await;
        let second = &mut client.read_next_packet().await;
        if let Ok(Some(LoginStart(body))) = second {
            let response_spec = LoginSetCompressionSpec {
                threshold: mcproto_rs::types::VarInt::from(compression_threshold),
            };
            let mut result = (body.name.clone(), UUID4::random());
            if self.online {
                let server_id = "                ".to_string();
                let public_key: &mut [u8] = &mut [0; 16];
                for mut _i in public_key.iter() {
                    _i = &rand::random::<u8>();
                }
                let verify_token: &mut [u8] = &mut [0; 16];
                for mut _i in verify_token.iter() {
                    _i = &rand::random::<u8>();
                }
                let encryption_spec = LoginEncryptionRequestSpec {
                    server_id: server_id.clone(),
                    public_key: CountedArray::from(public_key.to_vec()),
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
                        if response.verify_token == CountedArray::from(verify_token.to_vec()) {
                            let verify = auth::verify_join(
                                &body.name,
                                server_id,
                                &response.shared_secret,
                                public_key,
                            )
                            .await;
                            if let Ok(verified) = verify {
                                if let Err(error) =
                                    client.enable_encryption(public_key, verify_token)
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
    async fn handle_status(
        &mut self,
        client_mutex: Arc<Mutex<MinecraftConnection>>,
    ) -> anyhow::Result<()> {
        use super::Packet::{StatusPing, StatusPong, StatusRequest};
        use mcproto_rs::status::StatusPlayerSampleSpec;
        use proto::StatusPongSpec;
        let client = &mut client_mutex.lock().await;
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
                if let Err(error) = self.status.send_status(client).await {
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
    pub async fn login_kick(mut connection: MutexGuard<'_, MinecraftConnection>, message: Chat) -> Result<()> {
        use Packet::LoginDisconnect;
        use proto::LoginDisconnectSpec;
        let spec = LoginDisconnectSpec {
            message
        };
        connection.write_packet(LoginDisconnect(spec)).await
    }

    /// Kicks a player that is in the login state.
    ///
    /// # Arguments
    ///
    /// * `connection` Connection of the player to kick.
    /// * `reason` Reason for the kick.
    pub async fn play_kick(mut connection: MutexGuard<'_, MinecraftConnection>, reason: Chat) -> Result<()> {
        use Packet::PlayDisconnect;
        use proto::PlayDisconnectSpec;
        let spec = PlayDisconnectSpec {
            reason
        };
        connection.write_packet(PlayDisconnect(spec)).await
    }

    /// Send a message to a client (can be sent to multiple locations on client screen).
    ///
    /// # Arguments
    ///
    /// * `connection` Arc-Mutex containing the client that the message should be sent to.
    /// * `message` Chat message to send.
    /// * `position` where the message should be displayed on the client's screen.
    pub async fn send_message(
        mut connection: MutexGuard<'_, MinecraftConnection>,
        message: Chat,
        position: ChatPosition,
    ) -> Result<()> {
        use super::Packet::PlayServerChatMessage;
        use proto::PlayServerChatMessageSpec;
        let spec = PlayServerChatMessageSpec {
            message,
            sender: UUID4::random(),
            position,
        };
        let packet = PlayServerChatMessage(spec);
        connection.write_packet(packet).await
    }

    /// Send a chat message to all connected clients.
    ///
    /// # Arguments
    ///
    /// * `message` Chat message to send.
    #[allow(unused_must_use)]
    pub async fn broadcast_chat(&mut self, message: Chat) {
        for player in self.players.clone().lock().await.values() {
            Self::send_message(
                player.connection.lock().await,
                message.clone(),
                proto::ChatPosition::ChatBox,
            )
            .await;
        }
    }

    /// Handle packets sent by connected clients.
    async fn handle_packet(&mut self, packet: Packet, sender: Arc<ServerClient>) {
        match packet {
            Packet::PlayClientChatMessage(body) => {
                self.broadcast_chat(Chat::from_traditional(
                    &("<".to_owned() + sender.name.as_str() + "> " + body.message.as_str()),
                    true,
                ))
                .await;
            }
            _ => {}
        }
    }
}

/// Represents a connected client/player.
#[allow(dead_code)]
struct ServerClient {
    name: String,
    uuid: UUID4,
    connection: Arc<Mutex<MinecraftConnection>>,
}
