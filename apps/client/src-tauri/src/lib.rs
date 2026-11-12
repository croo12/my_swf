use serde_json::Value;
use std::time::Duration;

#[tauri::command]
async fn factory_request(
    api_base: String,
    token: String,
    path: String,
    method: String,
    body: Option<Value>,
) -> Result<Value, String> {
    if token.trim().len() < 24 { return Err("Factory API 토큰을 입력하세요.".into()); }
    if !path.starts_with("/api/") || path.contains("..") || path.contains('#') || path.contains('?') {
        return Err("허용되지 않은 API 경로입니다.".into());
    }
    let base = reqwest::Url::parse(&api_base).map_err(|e|format!("서버 URL: {e}"))?;
    let safe_http_local = base.scheme() == "http" && matches!(base.host_str(), Some("localhost" | "127.0.0.1"));
    if !(base.scheme() == "https" || safe_http_local) || base.username() != "" || base.password().is_some() || base.query().is_some() || base.fragment().is_some() {
        return Err("서버는 HTTPS 주소를 사용하세요. HTTP는 localhost에서만 허용됩니다.".into());
    }
    let origin = format!("{}://{}{}",base.scheme(),base.host_str().ok_or("호스트가 필요합니다")?,base.port().map(|p|format!(":{p}")).unwrap_or_default());
    let url = format!("{origin}{path}");
    let client = reqwest::Client::builder().timeout(Duration::from_secs(35)).build().map_err(|e|e.to_string())?;
    let request = match method.as_str() {
        "GET" => client.get(url),
        "POST" => client.post(url),
        "PUT" => client.put(url),
        _ => return Err("Unsupported method".into()),
    }.bearer_auth(token);
    let request = if let Some(v) = body {request.json(&v)} else {request};
    let response = request.send().await.map_err(|e|format!("연결 실패: {e}"))?;
    let status = response.status();
    let text = response.text().await.map_err(|e|e.to_string())?;
    if !status.is_success() {return Err(format!("HTTP {status}: {}",text.chars().take(500).collect::<String>()));}
    serde_json::from_str::<Value>(&text).map_err(|e|format!("잘못된 서버 응답: {e}"))
}

// The API token is kept in the OS credential store so it survives app restarts without being
// written to disk in plain text. Platforms without a store here (Android) stay session-only.
#[cfg(any(target_os = "windows", target_os = "macos"))]
mod token_store {
    const SERVICE: &str = "dev.softwarefactory.planning";
    const ACCOUNT: &str = "factory-api-token";

    fn entry() -> Result<keyring::Entry, String> {
        keyring::Entry::new(SERVICE, ACCOUNT).map_err(|e| e.to_string())
    }
    pub fn load() -> Result<Option<String>, String> {
        match entry()?.get_password() {
            Ok(token) => Ok(Some(token)),
            Err(keyring::Error::NoEntry) => Ok(None),
            Err(e) => Err(e.to_string()),
        }
    }
    pub fn save(token: &str) -> Result<(), String> {
        if token.is_empty() {
            return match entry()?.delete_credential() {
                Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
                Err(e) => Err(e.to_string()),
            };
        }
        entry()?.set_password(token).map_err(|e| e.to_string())
    }
}
#[cfg(not(any(target_os = "windows", target_os = "macos")))]
mod token_store {
    pub fn load() -> Result<Option<String>, String> { Ok(None) }
    pub fn save(_token: &str) -> Result<(), String> { Ok(()) }
}

#[tauri::command]
async fn load_token() -> Result<Option<String>, String> { token_store::load() }

#[tauri::command]
async fn save_token(token: String) -> Result<(), String> { token_store::save(token.trim()) }

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![factory_request, load_token, save_token])
        .run(tauri::generate_context!())
        .expect("error while running Software Factory");
}
