use std::sync::Arc;

use tokio::{net::TcpListener, sync::Mutex};

use super::net::connection::MinecraftConnection;
use super::proto;
use anyhow::Result;
use mcproto_rs::uuid::UUID4;
use mcproto_rs::{status::StatusSpec, types::Chat};

pub struct Server {
    connections: Arc<Mutex<Vec<Arc<Mutex<MinecraftConnection>>>>>,
    bind_address: String,
    status: Option<StatusSpec>,
}

impl Server {
    pub fn new(bind_address: String) -> Self {
        Server {
            connections: Arc::new(Mutex::new(vec![])),
            bind_address,
            status: None,
        }
    }

    pub async fn start(&mut self) {
        let connections = self.connections.clone();
        let address = self.bind_address.clone();
        let mut listener = TcpListener::bind(address).await;
        if let Ok(listener) = &mut listener {
            loop {
                if let Ok((socket, address)) = listener.accept().await {
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
                            if let Ok(_) = self.handle_login(client_arc.clone(), 256).await {
                                println!("{} successfully logged in.", address.to_string())
                            } else {
                                println!("{} failed to log in.", address.to_string())
                            }
                        } else {
                            if let Ok(_) = self.handle_status(client_arc.clone()).await  {
                                println!("{} successfully got server status.", address.to_string())
                            } else {
                                println!("{} failed to lget server status.", address.to_string())
                            }
                        }
                    } else {
                        println!(
                            "Handshake with {} failed: {}",
                            address.to_string(),
                            handshake.err().unwrap()
                        )
                    }
                    connections.lock().await.push(client_arc);
                }
            }
        }
    }

    pub async fn handle_login(
        &mut self,
        client_mutex: Arc<Mutex<MinecraftConnection>>,
        compression_threshold: i32,
    ) -> Result<()> {
        use super::Packet::{LoginSetCompression, LoginStart, LoginSuccess};
        use proto::{LoginSetCompressionSpec, LoginSuccessSpec};
        let client = &mut client_mutex.lock().await;
        let second = &mut client.read_next_packet().await;
        if let Ok(second) = second {
            if let Some(LoginStart(body)) = second {
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
                if let Err(error) = client
                    .write_packet(LoginSuccess(LoginSuccessSpec {
                        username: body.name.clone(),
                        uuid: UUID4::random(),
                    }))
                    .await
                {
                    return Err(error);
                }
            }
            return Ok(());
        } else {
            return Err(anyhow::anyhow!(
                "Client did not follow up with status request."
            ));
        }
    }

    pub async fn handle_status(
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
                            name: "rust-mc".to_string(),
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
}
