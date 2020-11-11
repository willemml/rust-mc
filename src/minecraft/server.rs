use std::sync::Arc;

use tokio::{net::TcpListener, runtime::Runtime, sync::Mutex};

use super::{connection::MinecraftConnection, proto, Packet};
use anyhow::Result;
use mcproto_rs::{status::StatusSpec, types::Chat, uuid::UUID4};
use proto::ChatPosition;

/// Represents a Minecraft server.
pub struct Server {
    connections: Arc<Mutex<Vec<Arc<ServerClient>>>>,
    bind_address: String,
    status: Option<StatusSpec>,
    runtime: Runtime,
}

impl Server {
    /// Create a new server based on an address.
    pub fn new(bind_address: String) -> Self {
        Server {
            connections: Arc::new(Mutex::new(vec![])),
            bind_address,
            status: None,
            runtime: Runtime::new().unwrap(),
        }
    }

    /// Starts listening for tcp connections on `self.bind_address`.
    /// When a client connects the server performs the login sequence.
    pub async fn start(self) -> Arc<Mutex<Self>> {
        let connections = self.connections.clone();
        let address = self.bind_address.clone();
        let mut listener = TcpListener::bind(address).await;
        let self_mutex = Arc::new(Mutex::new(self));
        if let Ok(listener) = &mut listener {
            loop {
                if let Ok((socket, address)) = listener.accept().await {
                    let self_join_arc = self_mutex.clone();
                    let connections = connections.clone();
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
                                    login = self_join_arc
                                        .lock()
                                        .await
                                        .handle_login(client_arc.clone(), 256)
                                        .await;
                                }
                                if let Ok(login) = login {
                                    let server_client = Arc::new(ServerClient {
                                        name: login.0,
                                        uuid: login.1,
                                        connection: client_arc.clone()
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
                                                        .handle_packet(packet, server_client_arc.clone())
                                                        .await;
                                                }
                                            };
                                        }
                                    };
                                    self_join_arc.lock().await.runtime.spawn(packet_loop);
                                    connections.lock().await.push(server_client);
                                    println!("{} successfully logged in.", address.to_string());
                                } else {
                                    println!("{} failed to log in.", address.to_string())
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
                    self_mutex.lock().await.runtime.spawn(join);
                }
            }
        };
        self_mutex
    }

    /// Handle client login.
    async fn handle_login(
        &mut self,
        client_mutex: Arc<Mutex<MinecraftConnection>>,
        compression_threshold: i32,
    ) -> Result<(String, UUID4)> {
        use super::Packet::{LoginSetCompression, LoginStart, LoginSuccess};
        use mcproto_rs::protocol::State::Play;
        use proto::{LoginSetCompressionSpec, LoginSuccessSpec};
        let client = &mut client_mutex.lock().await;
        let second = &mut client.read_next_packet().await;
        if let Ok(Some(LoginStart(body))) = second {
            let response_spec = LoginSetCompressionSpec {
                threshold: mcproto_rs::types::VarInt::from(compression_threshold),
            };
            if let Err(error) = client
                .write_packet(LoginSetCompression(response_spec))
                .await
            {
                return Err(error);
            } else {
                client.set_compression_threshold(compression_threshold);
            }
            let result = (body.name.clone(), UUID4::random());
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
                "Client did not follow up with status request."
            ));
        }
    }

    /// Handle status requests by a client.
    async fn handle_status(
        &mut self,
        client_mutex: Arc<Mutex<MinecraftConnection>>,
    ) -> anyhow::Result<()> {
        use super::Packet::{StatusPing, StatusPong, StatusRequest, StatusResponse};
        use mcproto_rs::status::{StatusPlayerSampleSpec, StatusPlayersSpec, StatusVersionSpec};
        use proto::{StatusPongSpec, StatusResponseSpec};
        let client = &mut client_mutex.lock().await;
        let second = &mut client.read_next_packet().await;
        if let Ok(second) = second {
            if let Some(StatusRequest(_)) = second {
                let status = if let Some(status_spec) = self.status.clone() {
                    status_spec
                } else {
                    StatusSpec {
                        description: Chat::from_text(
                            "Welcome to rust-mc, a Minecraft server and client written in rust!",
                        ),
                        version: StatusVersionSpec {
                            name: "rust-mc 1.16.3".to_string(),
                            protocol: 753,
                        },
                        players: StatusPlayersSpec {
                            max: 20,
                            online: 10,
                            sample: vec![StatusPlayerSampleSpec {
                                id: UUID4::random(),
                                name: "".to_string(),
                            }],
                        },
                        favicon: None,
                    }
                };
                let response_spec = StatusResponseSpec { response: status };
                if let Err(error) = client.write_packet(StatusResponse(response_spec)).await {
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

    /// Send a chat message to a client.
    pub async fn send_chat_message(
        connection: tokio::sync::MutexGuard<'_, MinecraftConnection>,
        message: Chat,
    ) -> Result<()> {
        Self::send_message(connection, message, ChatPosition::ChatBox).await
    }

    /// Send a message to a client (can be sent to multiple locations on client screen).
    pub async fn send_message(
        mut connection: tokio::sync::MutexGuard<'_, MinecraftConnection>,
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

    #[allow(unused_must_use)]
    pub async fn broadcast_chat(&mut self, message: Chat) {
        for player in self.connections.clone().lock().await.iter() {
            Self::send_chat_message(player.connection.lock().await, message.clone()).await;
        }
    }

    /// Handle packets sent by connected clients.
    async fn handle_packet(&mut self, packet: Packet, sender: Arc<ServerClient>) {
        match packet {
            Packet::PlayClientChatMessage(body) => {
                self.broadcast_chat(Chat::from_traditional(&("<".to_owned() + sender.name.as_str() + "> " + body.message.as_str()), true))
                    .await;
            }
            _ => {}
        }
    }
}

#[allow(dead_code)]
struct ServerClient {
    name: String,
    uuid: UUID4,
    connection: Arc<Mutex<MinecraftConnection>>,
}
