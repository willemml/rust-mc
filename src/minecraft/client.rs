use super::{net::connection::ServerConnection, proto, Packet};
use anyhow::Result;
use mcproto_rs::{protocol::State, types::CountedArray};
use std::net::SocketAddr;

const SERVER_NONE_ERROR: &str = "Not connected to server.";
const WRONG_PACKET_ERROR: &str = "Recieved an unexpected packet.";

pub struct Client {
    address: SocketAddr,
    pub profile: crate::auth::Profile,
    pub(crate) server: Option<ServerConnection>,
    connected: bool,
}

impl Client {
    pub fn new(address: SocketAddr, profile: crate::auth::Profile) -> Client {
        Self {
            address,
            profile,
            server: None,
            connected: false,
        }
    }

    pub async fn connect(&mut self) -> Result<()> {
        let auth = self.profile.authenticate().await;
        if let Ok(_) = auth {
            if let Ok(connection) = ServerConnection::connect_async(self.address).await {
                self.server = Some(connection);
                if let Some(server) = &mut self.server {
                    if let Ok(_) = server
                        .handshake(
                            proto::HandshakeNextState::Login,
                            &self.profile.game_profile.name,
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

    pub async fn send_chat_message(&mut self, message: &String) -> Result<()> {
        let spec = proto::PlayClientChatMessageSpec { message: message.clone() };
        self.send_packet(Packet::PlayClientChatMessage(spec)).await
    }

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

    async fn send_packet(&mut self, packet: Packet) -> Result<()> {
        if let Some(server) = &mut self.server {
            server.write_packet(packet).await
        } else {
            Err(anyhow::anyhow!(SERVER_NONE_ERROR))
        }
    }

    fn set_state(&mut self, new_state: State) -> Result<()> {
        if let Some(server) = &mut self.server {
            server.set_state(new_state);
            Ok(())
        } else {
            Err(anyhow::anyhow!(SERVER_NONE_ERROR))
        }
    }

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

    pub async fn login(&mut self) -> Result<()> {
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
