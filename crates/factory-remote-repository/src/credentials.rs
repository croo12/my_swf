//! Retains the credential identity used by the original Tauri client.
pub fn load_token() -> Result<String, String> {
    token(true, "")
}
pub fn save_token(value: &str) -> Result<(), String> {
    token(false, value).map(|_| ())
}

#[cfg(any(target_os = "windows", target_os = "macos"))]
fn token(load: bool, value: &str) -> Result<String, String> {
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
fn token(_load: bool, _value: &str) -> Result<String, String> {
    Ok(String::new())
}
