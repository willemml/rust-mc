/// Authentication API
pub mod auth;
/// Minecraft server ID hash methods
mod hash;

/// Post a JSON string to a URL.
async fn http_post_json(url: &str, json: String) -> std::result::Result<reqwest::Response, reqwest::Error> {
    let client = reqwest::Client::new();
    client
        .post(url)
        .header(reqwest::header::CONTENT_TYPE, "application/json")
        .body(json)
        .send()
        .await
}
