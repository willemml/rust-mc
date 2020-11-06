use anyhow::Result;
use mcproto_rs::uuid::UUID4;
use serde::{Deserialize, Serialize};
const SERVER_JOIN_URL: &str = "https://sessionserver.mojang.com/session/minecraft/join";
const PROFILE_LOGIN_URL: &str = "https://authserver.mojang.com/authenticate";

#[derive(Clone)]
pub struct Profile {
    username: String,
    password: String,
    access_token: String,
    pub offline: bool,
    pub game_profile: MinecraftProfile,
}

impl Profile {
    pub fn new(username: &str, password: &str, offline: bool) -> Self {
        let game_profile = MinecraftProfile {
            id: UUID4::random(),
            name: username.to_string(),
        };
        Self {
            username: username.to_string(),
            password: password.to_string(),
            access_token: "".to_string(),
            offline,
            game_profile,
        }
    }

    pub async fn authenticate(&mut self) -> Result<()> {
        if self.offline {
            return Ok(());
        };
        if !self.username.is_empty() {
            if !self.password.is_empty() {
                let auth_request = AuthenticateRequest {
                    agent: Agent {
                        name: "Minecraft".to_string(),
                        version: 1,
                    },
                    username: self.username.clone(),
                    password: self.password.clone(),
                };
                if let Ok(json) = serde_json::to_string(&auth_request) {
                    let client = reqwest::Client::new();
                    let result = client
                        .post(PROFILE_LOGIN_URL)
                        .header(reqwest::header::CONTENT_TYPE, "application/json")
                        .body(json)
                        .send()
                        .await;
                    if let Ok(response) = result {
                        if let Ok(text) = response.text().await {
                            let response =
                                serde_json::from_str::<AuthenticateResponse>(text.as_str());
                            if let Ok(response) = response {
                                self.access_token = response.accessToken;
                                self.game_profile = response.selectedProfile;
                                Ok(())
                            } else {
                                Err(anyhow::anyhow!("Invalid response JSON."))
                            }
                        } else {
                            Err(anyhow::anyhow!("Empty response."))
                        }
                    } else {
                        Err(anyhow::anyhow!("Failed to send request."))
                    }
                } else {
                    Err(anyhow::anyhow!(
                        "Failed to serialize authentication request."
                    ))
                }
            } else {
                Err(anyhow::anyhow!("No password provided"))
            }
        } else {
            Err(anyhow::anyhow!("Username is empty."))
        }
    }

    pub async fn join_server(&self, server_hash: String) -> Result<()> {
        if self.offline {
            return Err(anyhow::anyhow!(
                "Cannot join online-mode server with offline account."
            ));
        }
        if !self.access_token.is_empty() {
            let join_request = JoinRequest {
                accessToken: self.access_token.clone(),
                selectedProfile: self.game_profile.id.to_string().replace("-", ""),
                serverId: server_hash,
            };
            if let Ok(json) = serde_json::to_string(&join_request) {
                let client = reqwest::Client::new();
                let result = client
                    .post(SERVER_JOIN_URL)
                    .header(reqwest::header::CONTENT_TYPE, "application/json")
                    .body(json)
                    .send()
                    .await;
                if let Ok(response) = result {
                    if response.status() == reqwest::StatusCode::NO_CONTENT {
                        Ok(())
                    } else {
                        Err(anyhow::anyhow!("Failed to authenticate with Mojang."))
                    }
                } else {
                    Err(anyhow::anyhow!("Failed to send join request."))
                }
            } else {
                Err(anyhow::anyhow!("Failed serialize request to JSON."))
            }
        } else {
            Err(anyhow::anyhow!("Not authenticated."))
        }
    }
}

#[derive(Serialize, Deserialize, Clone)]
pub struct MinecraftProfile {
    pub name: String,
    pub id: UUID4,
}

#[derive(Serialize, Deserialize, Clone)]
struct AuthenticateResponse {
    accessToken: String,
    clientToken: String,
    availableProfiles: Vec<MinecraftProfile>,
    selectedProfile: MinecraftProfile,
}

#[derive(Serialize, Deserialize, Clone)]
struct Agent {
    name: String,
    version: i8,
}

#[derive(Serialize, Deserialize, Clone)]
struct AuthenticateRequest {
    agent: Agent,
    username: String,
    password: String,
}

#[derive(Serialize, Deserialize, Clone)]
struct JoinRequest {
    accessToken: String,
    selectedProfile: String,
    serverId: String,
}
