use anyhow::Result;
use mcproto_rs::uuid::UUID4;
use serde::{Deserialize, Serialize};

const HAS_JOINED_SERVER_URL: &str = "https://sessionserver.mojang.com/session/minecraft/hasJoined";
const JOIN_SERVER_URL: &str = "https://sessionserver.mojang.com/session/minecraft/join";
const AUTHENTICATE_URL: &str = "https://authserver.mojang.com/authenticate";
const INVALIDATE_URL: &str = "https://authserver.mojang.com/invalidate";
const VALIDATE_URL: &str = "https://authserver.mojang.com/validate";
const SIGNOUT_URL: &str = "https://authserver.mojang.com/signout";
const REFRESH_URL: &str = "https://authserver.mojang.com/refresh";

/// A Minecraft player user account.
pub struct Profile {
    /// Username or email of the Minecraft account.
    username: String,
    /// Password of the Minecraft account.
    password: String,
    /// Access token used when connecting to Minecraft servers.
    access_token: String,
    /// Client token used to refresh the access token without using the username and password again.
    client_token: String,
    /// Whether or not this account authenticates with Mojang (true for no, false for yes).
    pub offline: bool,
    /// The information on the Minecraft player attached to this account.
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
                clientToken: self.client_token.clone(),
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
                clientToken: self.client_token.clone(),
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
                password: self.password.clone(),
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
                    clientToken: self.client_token.clone(),
                };
                if let Ok(json) = serde_json::to_string(&signout_payload) {
                    if let Ok(response) = super::http_post_json(REFRESH_URL, json).await {
                        if let Ok(text) = response.text().await {
                            if let Ok(refresh) =
                                serde_json::from_str::<RefreshResponse>(text.as_str())
                            {
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
                Err(anyhow::anyhow!(
                    "Cannot refresh without a client token and an access token."
                ))
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
    pub async fn join_server(
        &self,
        server_id: String,
        shared_secret: &[u8],
        public_key: &[u8],
    ) -> Result<()> {
        if self.offline {
            return Err(anyhow::anyhow!(
                "Cannot join online-mode server with offline account."
            ));
        }
        if !self.access_token.is_empty() {
            let join_request = JoinRequest {
                accessToken: self.access_token.clone(),
                selectedProfile: self.game_profile.id.to_string().replace("-", ""),
                serverId: super::hash::calc_hash(&server_id, shared_secret, public_key),
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

/// Checks that a player has authenticated with Mojang before joining server.
///
/// # Arguments
///
/// * `username` username of the player to verify.
/// * `server_id` String containing the ID of the server tha player is trying to join.
/// * `shared_secret` The shared secret that the client generated.
/// * `public_key` The server's public key.
/// 
/// # Examples
///
/// ```rust
/// use rust_mc::mojang::auth::verify_join;
///
/// let username = "rust_mc";
/// let server_id = "16to20charstring".to_string();
/// let shared_secret: &[u8] = &[0; 16];
/// let public_key: &[u8] = &[0; 16];
///
/// if let Ok((username, uuid)) = verify_join(username, server_id, shared_secret, public_key).await {
///     println!("{} has successfully authenticated with Mojang and their UUID is {}.", username, uuid.to_string());
/// }
/// ```
pub async fn verify_join(
    username: &str,
    server_id: String,
    shared_secret: &[u8],
    public_key: &[u8],
) -> Result<(String, UUID4)> {
    let client = reqwest::Client::new();
    let response = client
        .get((HAS_JOINED_SERVER_URL.to_owned() + "?username=" + username + "&serverId=" + super::hash::calc_hash(&server_id, shared_secret, public_key).as_str()).as_str())
        .send()
        .await;
    if let Ok(response) = response {
        if let Ok(text) = response.text().await {
            if let Ok(join) = serde_json::from_str::<ServerJoinResponse>(&text) {
                return Ok((join.name, join.id));
            }
            return Err(anyhow::anyhow!("Bad response."))
        }
        return Err(anyhow::anyhow!("Empty response."))
    };
    return Err(anyhow::anyhow!("Failed to send request."))
}

/// Minecraft player ID data.
#[derive(Serialize, Deserialize, Clone)]
pub struct MinecraftProfile {
    /// Username of the player.
    pub name: String,
    /// UUID of the player.
    pub id: UUID4,
}

/// Response to a request to authenticate with Mojang.
#[allow(non_snake_case)]
#[derive(Serialize, Deserialize, Clone)]
struct AuthenticateResponse {
    accessToken: String,
    clientToken: String,
    availableProfiles: Vec<MinecraftProfile>,
    selectedProfile: MinecraftProfile,
}

/// Payload containg a client token and an access token.
#[allow(non_snake_case)]
#[derive(Serialize, Deserialize, Clone)]
struct ClientAccessTokenPayload {
    accessToken: String,
    clientToken: String,
}

/// Response to a request to refresh an access token.
#[allow(non_snake_case)]
#[derive(Serialize, Deserialize, Clone)]
struct RefreshResponse {
    accessToken: String,
    clientToken: String,
    selectedProfile: MinecraftProfile,
}

/// Payload sent to invalidate all access tokens associated with an account.
#[allow(non_snake_case)]
#[derive(Serialize, Deserialize, Clone)]
struct SignoutPayload {
    username: String,
    password: String,
}

/// Agent, game type and the version of the agent.
#[allow(non_snake_case)]
#[derive(Serialize, Deserialize, Clone)]
struct Agent {
    name: String,
    version: i8,
}

/// Request sent to ask for an access token to an account using a username and password.
#[allow(non_snake_case)]
#[derive(Serialize, Deserialize, Clone)]
struct AuthenticateRequest {
    agent: Agent,
    username: String,
    password: String,
}

/// Request sent when joining a server.
#[allow(non_snake_case)]
#[derive(Serialize, Deserialize, Clone)]
struct JoinRequest {
    accessToken: String,
    selectedProfile: String,
    serverId: String,
}

/// Response to a request checking if a plyer is authenticated.
#[derive(Serialize, Deserialize, Clone)]
struct ServerJoinResponse {
    id: UUID4,
    name: String,
}
