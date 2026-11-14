use sha2::{Digest, Sha256};
use std::{env, fmt::Write, fs, path::PathBuf};

fn main() {
    let workspace = PathBuf::from(env::var_os("CARGO_MANIFEST_DIR").unwrap()).join("../..");
    let prepared = workspace.join("target/native-router");
    let target_os = env::var("CARGO_CFG_TARGET_OS").unwrap();
    let router = if target_os == "windows" {
        "libavoid-server.exe"
    } else {
        "libavoid-server"
    };
    let mut assets = vec![
        (router, prepared.join(router)),
        (
            "LICENSE-libavoid-server.txt",
            prepared.join("LICENSE-libavoid-server.txt"),
        ),
        (
            "LICENSE-libavoid.txt",
            prepared.join("LICENSE-libavoid.txt"),
        ),
        (
            "ROUTER-NOTICES.txt",
            workspace.join("docs/ROUTER-NOTICES.txt"),
        ),
    ];
    for name in [
        "libavoid-server-v0.3.1.tar.gz",
        "adaptagrams-fcac991.tar.gz",
        "prepare-router.ps1",
        "prepare-router.sh",
    ] {
        assets.push((name, prepared.join("SOURCES").join(name)));
    }
    let mut hash = Sha256::new();
    let mut generated = format!(
        "pub const ROUTER_NAME: &str = {router:?};\npub const ASSETS: &[(&str, &[u8])] = &[\n"
    );
    for (index, (name, source)) in assets.iter().enumerate() {
        println!("cargo:rerun-if-changed={}", source.display());
        let data = fs::read(source).unwrap_or_else(|error| panic!(
            "Cannot embed {}: {error}. Run scripts/native/prepare-router.ps1 (Windows) or prepare-router.sh (macOS) before building.", source.display()
        ));
        if index == 0 && target_os == "windows" {
            assert_eq!(
                format!("{:x}", Sha256::digest(&data)),
                "4676b8b4fd6f37ac2a74a5d8d11b023e45f34eb6bd8887f217bc776f4acad74d",
                "Unexpected Windows router payload"
            );
        }
        let relative = if index >= 4 {
            format!("SOURCES/{name}")
        } else {
            name.to_string()
        };
        hash.update(relative.as_bytes());
        hash.update([0]);
        hash.update(&data);
        let absolute = fs::canonicalize(source).unwrap();
        writeln!(
            generated,
            "({relative:?}, include_bytes!({:?})),",
            absolute.to_str().unwrap()
        )
        .unwrap();
    }
    generated.push_str("];\n");
    writeln!(
        generated,
        "pub const PAYLOAD_ID: &str = \"{:x}\";",
        hash.finalize()
    )
    .unwrap();
    fs::write(
        PathBuf::from(env::var_os("OUT_DIR").unwrap()).join("router_payload.rs"),
        generated,
    )
    .unwrap();
}
