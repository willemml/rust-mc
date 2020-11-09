use anyhow::Result;
use mcproto_rs::uuid::UUID4;
use serde::{Deserialize, Serialize};

const JOIN_SERVER_URL: &str = "https://sessionserver.mojang.com/session/minecraft/join";
const AUTHENTICATE_URL: &str = "https://authserver.mojang.com/authenticate";
const INVALIDATE_URL: &str = "https://authserver.mojang.com/invalidate";
const VALIDATE_URL: &str = "https://authserver.mojang.com/validate";
const SIGNOUT_URL: &str = "https://authserver.mojang.com/signout";
const REFRESH_URL: &str = "https://authserver.mojang.com/refresh";

/// A Minecraft player user account.
pub struct Profile {
    username: String,
    password: String,
    access_token: String,
    client_token: String,
    pub offline: bool,
    pub game_profile: MinecraftProfile,
}

impl Profile {
    /// Returns a new Profile.
    ///
    /// # Arguments
    ///
    /// * `username` username of the Minecraft account to sign in to, if it is a mojang account use email, should never be empty.
    /// * `password` password of the account to use, if `offline` is false the contents of this don't matter, empty is recommended.
    /// * `offline` whether or not this Profile should authenticate with Mojang (true is no, false is yes).
    ///
    /// # Examples
    ///
    /// Online account that authenticates with Mojang:
    /// ```rust
    /// use rust_mc::mojang::auth::Profile;
    ///
    /// let profile = Profile::new("example@example.com", "super_secret_password", false);
    /// ```
    ///
    /// Offline account, does not authenticate with Mojang:
    /// ```rust
    /// use rust_mc::mojang::auth::Profile;
    ///
    /// let profile = Profile::new("rust_mc", "", true);
    /// ```
    pub fn new(username: &str, password: &str, offline: bool) -> Self {
        let game_profile = MinecraftProfile {
            id: UUID4::random(),
            name: username.to_string(),
        };
        Self {
            username: username.to_string(),
            password: password.to_string(),
            access_token: "".to_string(),
            client_token: "".to_string(),
            offline,
            game_profile,
        }
    }

    /// Authenticates with Mojang using the `username` and `password` and gets player information.
    /// Only does this if `offline` is false, if it is true it does nothing and exits immediately and returns Ok(()).
    ///
    /// # Examples
    ///
    /// ```rust
    /// use rust_mc::mojang::auth::Profile;
    /// use futures::executor::block_on;
    ///
    /// let mut profile = Profile::new("rust_mc", "", true);
    ///
    /// block_on(profile.authenticate());
    /// ```
    pub async fn authenticate(&mut self) -> Result<()> {
        if self.offline {
            Ok(())
        } else {
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
                        let result = super::http_post_json(AUTHENTICATE_URL, json).await;
                        if let Ok(response) = result {
                            if let Ok(text) = response.text().await {
                                let response =
                                    serde_json::from_str::<AuthenticateResponse>(text.as_str());
                                if let Ok(response) = response {
                                    self.access_token = response.accessToken;
                                    self.game_profile = response.selectedProfile;
                                    Ok(())
                                } else {
                                    Err(anyhow::anyhow!("Failed to parse response."))
                                }
                            } else {
                                Err(anyhow::anyhow!("Empty response."))
                            }
                        } else {
                            Err(anyhow::anyhow!("Failed to post request."))
                        }
                    } else {
                        Err(anyhow::anyhow!("Failed to serialize request."))
                    }
                } else {
                    Err(anyhow::anyhow!("No password provided"))
                }
            } else {
                Err(anyhow::anyhow!("Username is empty."))
            }
        }
    }

    /// Asks Mojang if the current access token is valid.
    /// Only does this if `offline` is false, if it is true it does nothing and exits immediately and returns Ok(()).
    ///
    /// # Examples
    ///
    /// ```rust
    /// use rust_mc::mojang::auth::Profile;
    /// use futures::executor::block_on;
    ///
    /// let mut profile = Profile::new("rust_mc", "", true);
    ///
    /// block_on(profile.validate());
    /// ```
    pub async fn validate(&self) -> Result<bool> {
        if self.offline {
            Ok(true)
        } else {
            let validate_payload = ClientAccessTokenPayload {
                accessToken: self.access_token.clone(),
                clientToken: self.client_token.clone()
            };
            if let Ok(json) = serde_json::to_string(&validate_payload) {
                let result = super::http_post_json(VALIDATE_URL, json).await;
                if let Ok(response) = result {
                    if response.status() == reqwest::StatusCode::NO_CONTENT {
                        Ok(true)
                    } else if response.status() == reqwest::StatusCode::FORBIDDEN {
                        Ok(false)
                    } else {
                        Err(anyhow::anyhow!("Unexpected response status code."))
                    }
                } else {
                    Err(anyhow::anyhow!("Failed to post request"))
                }
            } else {
                Err(anyhow::anyhow!("Failed to serialize request."))
            }
        }
    }

    /// Asks Mojang to invalidate the current access token.
    /// Only does this if `offline` is false, if it is true it does nothing and exits immediately and returns Ok(()).
    ///
    /// # Examples
    ///
    /// ```rust
    /// use rust_mc::mojang::auth::Profile;
    /// use futures::executor::block_on;
    ///
    /// let mut profile = Profile::new("rust_mc", "", true);
    ///
    /// block_on(profile.invalidate());
    /// ```
    pub async fn invalidate(&self) -> Result<()> {
        if self.offline {
            Ok(())
        } else {
            let invalidate_payload = ClientAccessTokenPayload {
                accessToken: self.access_token.clone(),
                clientToken: self.client_token.clone()
            };
            if let Ok(json) = serde_json::to_string(&invalidate_payload) {
                if let Ok(_) = super::http_post_json(INVALIDATE_URL, json).await {
                    Ok(())
                } else {
                    Err(anyhow::anyhow!("Failed to post request."))
                }
            } else {
                Err(anyhow::anyhow!("Failed to serialize request."))
            }
        }
    }

    /// Asks Mojang to invalidate all access tokens that have been given to this account.
    /// Only does this if `offline` is false, if it is true it does nothing and exits immediately and returns Ok(()).
    ///
    /// # Examples
    ///
    /// ```rust
    /// use rust_mc::mojang::auth::Profile;
    /// use futures::executor::block_on;
    ///
    /// let mut profile = Profile::new("rust_mc", "", true);
    ///
    /// block_on(profile.signout());
    /// ```
    pub async fn signout(&self) -> Result<()> {
        if self.offline {
            Ok(())
        } else {
            let signout_payload = SignoutPayload {
                username: self.username.clone(),
                password: self.password.clone()
            };
            if let Ok(json) = serde_json::to_string(&signout_payload) {
                if let Ok(_) = super::http_post_json(SIGNOUT_URL, json).await {
                    Ok(())
                } else {
                    Err(anyhow::anyhow!("Failed to post request."))
                }
            } else {
                Err(anyhow::anyhow!("Failed to serialize request."))
            }
        }
    }

    /// Asks Mojang for a new access token based on the current access token and the current client token.
    /// Only does this if `offline` is false, if it is true it does nothing and exits immediately and returns Ok(()).
    ///
    /// # Examples
    ///
    /// ```rust
    /// use rust_mc::mojang::auth::Profile;
    /// use futures::executor::block_on;
    ///
    /// let mut profile = Profile::new("rust_mc", "", true);
    ///
    /// block_on(profile.authenticate();)
    /// ```
    pub async fn refresh(&mut self) -> Result<()> {
        if self.offline {
            Ok(())
        } else {
            if !(self.client_token.is_empty() && self.access_token.is_empty()) {
                let signout_payload = ClientAccessTokenPayload {
                    accessToken: self.access_token.clone(),
                    clientToken: self.client_token.clone()
                };
                if let Ok(json) = serde_json::to_string(&signout_payload) {
                    if let Ok(response) = super::http_post_json(REFRESH_URL, json).await {
                        if let Ok(text) = response.text().await {
                            if let Ok(refresh) = serde_json::from_str::<RefreshResponse>(text.as_str()) {
                                self.access_token = refresh.accessToken;
                                self.client_token = refresh.clientToken;
                                Ok(())
                            } else {
                                Err(anyhow::anyhow!("Failed to parse response."))
                            }
                        } else {
                            Err(anyhow::anyhow!("Empty response."))
                        }
                    } else {
                        Err(anyhow::anyhow!("Failed to post request."))
                    }
                } else {
                    Err(anyhow::anyhow!("Failed to serialize request."))
                }
            } else {
                Err(anyhow::anyhow!("Cannot refresh without a client token and an access token."))
            }
        }
    }

    /// Tells Mojang that this account is joining a server so that the server can verify that the account is valid.
    /// Only does this if `offline` is false, if it is true it does nothing and exits immediately and returns an error.
    ///
    /// # Arguments
    ///
    /// * `server_id` ID string of the server that is being joined.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use rust_mc::mojang::auth::Profile;
    /// use rust_mc::mojang::hash::calc_hash;
    /// use futures::executor::block_on;
    ///
    /// let mut profile = Profile::new("rust_mc", "", true);
    ///
    /// let server_id = calc_hash("server_id_string_here") // Server ID string, usually sent in the Encryption request packet.
    ///
    /// block_on(profile.join_server(server_id));
    /// ```
    pub async fn join_server(&self, server_id: String) -> Result<()> {
        if self.offline {
            return Err(anyhow::anyhow!(
                "Cannot join online-mode server with offline account."
            ));
        }
        if !self.access_token.is_empty() {
            let join_request = JoinRequest {
                accessToken: self.access_token.clone(),
                selectedProfile: self.game_profile.id.to_string().replace("-", ""),
                serverId: super::hash::calc_hash(&server_id),
            };
            if let Ok(json) = serde_json::to_string(&join_request) {
                let result = super::http_post_json(JOIN_SERVER_URL, json).await;
                if let Ok(response) = result {
                    if response.status() == reqwest::StatusCode::NO_CONTENT {
                        Ok(())
                    } else {
                        Err(anyhow::anyhow!("Failed to authenticate with Mojang."))
                    }
                } else {
                    Err(anyhow::anyhow!("Failed to post request."))
                }
            } else {
                Err(anyhow::anyhow!("Failed to serialize request."))
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

#[allow(non_snake_case)]
#[derive(Serialize, Deserialize, Clone)]
struct AuthenticateResponse {
    accessToken: String,
    clientToken: String,
    availableProfiles: Vec<MinecraftProfile>,
    selectedProfile: MinecraftProfile,
}

#[allow(non_snake_case)]
#[derive(Serialize, Deserialize, Clone)]
struct ClientAccessTokenPayload {
    accessToken: String,
    clientToken: String,
}

#[allow(non_snake_case)]
#[derive(Serialize, Deserialize, Clone)]
struct RefreshResponse {
    accessToken: String,
    clientToken: String,
    selectedProfile: MinecraftProfile,
}

#[allow(non_snake_case)]
#[derive(Serialize, Deserialize, Clone)]
struct SignoutPayload {
    username: String,
    password: String,
}

#[allow(non_snake_case)]
#[derive(Serialize, Deserialize, Clone)]
struct Agent {
    name: String,
    version: i8,
}

#[allow(non_snake_case)]
#[derive(Serialize, Deserialize, Clone)]
struct AuthenticateRequest {
    agent: Agent,
    username: String,
    password: String,
}

#[allow(non_snake_case)]
#[derive(Serialize, Deserialize, Clone)]
struct JoinRequest {
    accessToken: String,
    selectedProfile: String,
    serverId: String,
}
