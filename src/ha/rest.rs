use reqwest::Client;
use std::sync::OnceLock;
use url::Url;

static CLIENT: OnceLock<Client> = OnceLock::new();

fn client() -> &'static Client {
    CLIENT.get_or_init(|| {
        Client::builder()
            .timeout(std::time::Duration::from_secs(15))
            .build()
            .expect("reqwest client builds")
    })
}

/// Convert a Home Assistant WebSocket URL (ws:// or wss://) into the HTTP/HTTPS base
/// (http:// or https://) with path stripped to "/".
pub fn https_base_from_ws(ws_url: &str) -> Result<Url, String> {
    let url = Url::parse(ws_url).map_err(|e| e.to_string())?;
    let mut https = url.clone();
    let scheme = match url.scheme() {
        "ws" => "http",
        "wss" => "https",
        other => return Err(format!("unexpected scheme: {other}")),
    };
    https
        .set_scheme(scheme)
        .map_err(|_| "scheme replace failed".to_string())?;
    https.set_path("/");
    Ok(https)
}

pub async fn fetch_image_proxy(
    base_url: &str,
    entity_id: &str,
    token: &str,
) -> Result<Vec<u8>, String> {
    let base = https_base_from_ws(base_url)?;
    let url = base
        .join(&format!("api/image_proxy/{entity_id}"))
        .map_err(|e| e.to_string())?;
    fetch(url, token).await
}

pub async fn fetch_camera_proxy(
    base_url: &str,
    entity_id: &str,
    token: &str,
) -> Result<Vec<u8>, String> {
    let base = https_base_from_ws(base_url)?;
    let url = base
        .join(&format!("api/camera_proxy/{entity_id}"))
        .map_err(|e| e.to_string())?;
    fetch(url, token).await
}

async fn fetch(url: Url, token: &str) -> Result<Vec<u8>, String> {
    let resp = client()
        .get(url)
        .bearer_auth(token)
        .send()
        .await
        .map_err(|e| e.to_string())?;
    if !resp.status().is_success() {
        return Err(format!("HTTP {}", resp.status()));
    }
    let bytes = resp.bytes().await.map_err(|e| e.to_string())?;
    Ok(bytes.to_vec())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ws_to_https() {
        let u = https_base_from_ws("wss://ha.example/api/websocket").unwrap();
        assert_eq!(u.scheme(), "https");
        assert_eq!(u.host_str(), Some("ha.example"));
    }

    #[test]
    fn ws_plaintext() {
        let u = https_base_from_ws("ws://ha.local:8123/api/websocket").unwrap();
        assert_eq!(u.scheme(), "http");
    }

    #[test]
    fn ws_invalid_scheme() {
        assert!(https_base_from_ws("ftp://ha.example/api/websocket").is_err());
    }
}
