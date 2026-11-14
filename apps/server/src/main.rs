mod api;
mod cli;
mod git;
mod legacy;
mod state;

use state::AppState;
use std::{collections::HashMap, env, path::PathBuf, sync::Arc};
use tokio::{fs, net::TcpListener, sync::Mutex};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = env::args().collect();
    if cli::run(&args)? {
        return Ok(());
    }
    let token = env::var("FACTORY_API_TOKEN")
        .map_err(|_| "Set FACTORY_API_TOKEN (a long random secret)")?;
    if token.len() < 24 {
        return Err("FACTORY_API_TOKEN must be at least 24 characters".into());
    }
    let repo =
        std::path::absolute(env::var("FACTORY_REPO_PATH").map_err(|_| "Set FACTORY_REPO_PATH")?)?;
    if !repo.is_dir() {
        return Err("FACTORY_REPO_PATH must be a directory".into());
    }
    let data_dir = PathBuf::from(env::var("FACTORY_DATA_DIR").unwrap_or_else(|_| "./data".into()));
    fs::create_dir_all(data_dir.join("approved")).await?;
    fs::create_dir_all(data_dir.join("jobs")).await?;
    // std::path::absolute, not canonicalize: on Windows canonicalize yields \\?\ verbatim paths that git and other CLIs reject.
    let data_dir = std::path::absolute(data_dir)?;
    let state = Arc::new(AppState {
        repository: repo,
        data_dir,
        herdr_bin: env::var("HERDR_BIN").unwrap_or_else(|_| "herdr".into()),
        api_token: token,
        guard: Arc::new(Mutex::new(())),
        jobs: Arc::new(Mutex::new(HashMap::new())),
    });
    let app = api::router(state);
    let addr = env::var("FACTORY_BIND").unwrap_or_else(|_| "127.0.0.1:8787".into());
    let listener = TcpListener::bind(&addr).await?;
    println!("Factory server bound to {addr}; expose through Tailscale Serve, not Funnel.");
    axum::serve(listener, app).await?;
    Ok(())
}
