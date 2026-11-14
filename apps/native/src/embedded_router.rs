//! All runtime routing code, notices and corresponding upstream sources travel inside the app.
use std::{
    fs::{self, OpenOptions},
    io::Write,
    path::{Path, PathBuf},
    sync::{
        OnceLock,
        atomic::{AtomicU64, Ordering},
    },
};

include!(concat!(env!("OUT_DIR"), "/router_payload.rs"));

static PREPARED: OnceLock<PathBuf> = OnceLock::new();
static TEMP_ID: AtomicU64 = AtomicU64::new(0);

pub fn executable() -> Result<PathBuf, String> {
    // Advanced replacement remains available independently of the bundled application.
    if let Some(path) = std::env::var_os("FACTORY_LIBAVOID_SERVER") {
        return Ok(path.into());
    }
    let directory = if let Some(path) = PREPARED.get() {
        path.clone()
    } else {
        let path = cache_directory()?;
        materialize(&path)
            .map_err(|error| format!("내장 연결선 계산기를 준비하지 못했습니다: {error}"))?;
        let _ = PREPARED.set(path.clone());
        path
    };
    let (name, bytes) = ASSETS[0];
    let executable = directory.join(name);
    // Validate the cached executable against the embedded bytes, repairing missing/corrupt files.
    write_asset(&executable, bytes, true)
        .map_err(|error| format!("내장 연결선 계산기를 준비하지 못했습니다: {error}"))?;
    Ok(executable)
}

fn cache_directory() -> Result<PathBuf, String> {
    let base = if cfg!(windows) {
        std::env::var_os("LOCALAPPDATA").map(PathBuf::from)
    } else if cfg!(target_os = "macos") {
        std::env::var_os("HOME").map(|path| PathBuf::from(path).join("Library/Caches"))
    } else {
        std::env::var_os("XDG_CACHE_HOME")
            .map(PathBuf::from)
            .or_else(|| std::env::var_os("HOME").map(|path| PathBuf::from(path).join(".cache")))
    }
    .ok_or("앱 실행에 필요한 사용자 캐시 폴더를 찾지 못했습니다.")?;
    Ok(base
        .join("SoftwareFactoryNative")
        .join("router")
        .join(PAYLOAD_ID))
}

/// Exposes the complete bundled notices and rebuildable upstream sources without a network request.
pub fn export_sources(directory: &Path) -> Result<(), String> {
    for &(name, bytes) in ASSETS.iter().skip(1) {
        write_asset(&directory.join(name), bytes, false).map_err(|error| error.to_string())?;
    }
    Ok(())
}

fn materialize(directory: &Path) -> std::io::Result<()> {
    for &(name, bytes) in ASSETS {
        write_asset(&directory.join(name), bytes, name == ROUTER_NAME)?;
    }
    Ok(())
}

fn write_asset(path: &Path, bytes: &[u8], executable: bool) -> std::io::Result<()> {
    if fs::read(path).is_ok_and(|current| current == bytes) {
        return executable_permission(path, executable);
    }
    let parent = path.parent().unwrap();
    fs::create_dir_all(parent)?;
    let temporary = parent.join(format!(
        ".router-{}-{}.tmp",
        std::process::id(),
        TEMP_ID.fetch_add(1, Ordering::Relaxed)
    ));
    let result = (|| {
        let mut options = OpenOptions::new();
        options.write(true).create_new(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(if executable { 0o700 } else { 0o600 });
        }
        let mut file = options.open(&temporary)?;
        file.write_all(bytes)?;
        file.sync_all()?;
        drop(file);
        match fs::rename(&temporary, path) {
            Ok(()) => executable_permission(path, executable),
            Err(_) if fs::read(path).is_ok_and(|current| current == bytes) => {
                executable_permission(path, executable)
            }
            Err(error) => Err(error),
        }
    })();
    // Another app instance may have installed the same payload first.
    let _ = fs::remove_file(&temporary);
    result
}

fn executable_permission(path: &Path, executable: bool) -> std::io::Result<()> {
    #[cfg(unix)]
    if executable {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(path, fs::Permissions::from_mode(0o700))?;
    }
    #[cfg(not(unix))]
    let _ = (path, executable);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn standalone_payload_is_reusable_repairs_corruption_and_exports_sources() {
        let directory = tempfile::tempdir().unwrap();
        materialize(directory.path()).unwrap();
        let router = directory.path().join(ROUTER_NAME);
        let modified = router.metadata().unwrap().modified().unwrap();
        materialize(directory.path()).unwrap();
        assert_eq!(router.metadata().unwrap().modified().unwrap(), modified);
        fs::write(&router, b"corrupted cached router").unwrap();
        materialize(directory.path()).unwrap();
        assert_eq!(fs::read(router).unwrap(), ASSETS[0].1);
        let exported = directory.path().join("exported");
        export_sources(&exported).unwrap();
        assert!(!exported.join(ROUTER_NAME).exists());
        for &(name, bytes) in ASSETS.iter().skip(1) {
            assert_eq!(fs::read(exported.join(name)).unwrap(), bytes);
        }
    }

    #[test]
    fn concurrent_instances_can_prepare_the_same_payload() {
        let directory = tempfile::tempdir().unwrap();
        std::thread::scope(|scope| {
            for _ in 0..3 {
                scope.spawn(|| materialize(directory.path()).unwrap());
            }
        });
        for &(name, bytes) in ASSETS {
            assert_eq!(fs::read(directory.path().join(name)).unwrap(), bytes);
        }
    }
}
