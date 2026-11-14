//! Save and restore the workbench session using an explicit profile path.
use factory_plan::Plan;
use serde::{Deserialize, Serialize};
use std::{
    fs,
    path::{Path, PathBuf},
};

#[derive(Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Session {
    pub plan: Plan,
    pub repository: String,
    pub server: String,
}

pub fn default_file() -> Result<PathBuf, String> {
    let base = if cfg!(windows) {
        std::env::var_os("LOCALAPPDATA").map(PathBuf::from)
    } else if cfg!(target_os = "macos") {
        std::env::var_os("HOME").map(|home| PathBuf::from(home).join("Library/Application Support"))
    } else {
        std::env::var_os("XDG_CONFIG_HOME")
            .map(PathBuf::from)
            .or_else(|| std::env::var_os("HOME").map(|home| PathBuf::from(home).join(".config")))
    }
    .ok_or("사용자 저장 폴더를 찾지 못했습니다.")?;
    Ok(base.join("SoftwareFactoryNative").join("session.json"))
}

pub fn load(file: &Path) -> Result<Session, String> {
    match fs::read_to_string(file) {
        Ok(text) => serde_json::from_str(&text).map_err(|e| {
            format!(
                "{}: {e}. 원본을 보존하기 위해 자동 저장을 중지했습니다.",
                file.display()
            )
        }),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Session::default()),
        Err(e) => Err(format!("{}: {e}", file.display())),
    }
}

pub fn save(file: &Path, session: &Session) -> Result<(), String> {
    fs::create_dir_all(file.parent().unwrap()).map_err(|e| e.to_string())?;
    let text = serde_json::to_string_pretty(session).map_err(|e| e.to_string())?;
    let temp = file.with_extension("tmp");
    fs::write(&temp, text).map_err(|e| e.to_string())?;
    fs::rename(temp, file).map_err(|e| e.to_string())
}
