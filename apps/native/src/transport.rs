use factory_core::repository::Snapshot;
use std::time::Duration;

pub fn repository(base: &str, token: &str) -> Result<Snapshot, String> {
    let url = reqwest::Url::parse(base.trim()).map_err(|e| format!("서버 주소: {e}"))?;
    let local = url.scheme() == "http"
        && matches!(url.host_str(), Some("127.0.0.1" | "localhost" | "[::1]"));
    if !(url.scheme() == "https" || local)
        || !url.username().is_empty()
        || url.password().is_some()
        || url.query().is_some()
        || url.fragment().is_some()
        || !matches!(url.path(), "/" | "")
    {
        return Err("HTTPS 서버 주소를 입력하세요. HTTP는 localhost에서만 허용됩니다.".into());
    }
    if token.trim().len() < 24 {
        return Err("API 토큰은 24자 이상이어야 합니다.".into());
    }
    let client = reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(90))
        .redirect(reqwest::redirect::Policy::none())
        .build()
        .map_err(|e| e.to_string())?;
    let response = client
        .get(url.join("api/repository/tree").map_err(|e| e.to_string())?)
        .bearer_auth(token.trim())
        .send()
        .map_err(|e| e.to_string())?;
    let status = response.status();
    if !status.is_success() {
        return Err(if status == reqwest::StatusCode::NOT_FOUND {
            "서버에 계층 조회 API가 없습니다. 이 버전의 factory-server로 업데이트하세요.".into()
        } else {
            format!("서버 응답: {status}")
        });
    }
    response
        .json()
        .map_err(|e| format!("저장소 응답을 읽지 못했습니다: {e}"))
}

#[cfg(any(target_os = "windows", target_os = "macos"))]
pub fn token(load: bool, value: &str) -> Result<String, String> {
    // Same credential as the previous Tauri app; never serialize this into session.json.
    let entry = keyring::Entry::new("dev.softwarefactory.planning", "factory-api-token")
        .map_err(|e| e.to_string())?;
    if load {
        match entry.get_password() {
            Ok(token) => Ok(token),
            Err(keyring::Error::NoEntry) => Ok(String::new()),
            Err(e) => Err(e.to_string()),
        }
    } else {
        entry
            .set_password(value.trim())
            .map_err(|e| e.to_string())?;
        Ok(String::new())
    }
}
#[cfg(not(any(target_os = "windows", target_os = "macos")))]
pub fn token(_load: bool, _value: &str) -> Result<String, String> {
    Ok(String::new())
}
