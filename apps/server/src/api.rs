//! HTTP assembly and the read-only repository API used by the native app.
use crate::{
    git::git_state,
    state::{auth, err, ApiResult, AppState},
};
use axum::{
    extract::{DefaultBodyLimit, State},
    http::{HeaderMap, StatusCode},
    routing::get,
    Json, Router,
};
use factory_repository as repository;
use serde_json::json;
use std::sync::Arc;

pub(crate) fn router(state: Arc<AppState>) -> Router {
    Router::new()
        .route("/api/status", get(status))
        .route("/api/repository/tree", get(repository_tree))
        .merge(crate::legacy::router())
        .layer(DefaultBodyLimit::max(4 * 1024 * 1024))
        .with_state(state)
}

async fn status(State(s): State<Arc<AppState>>, headers: HeaderMap) -> ApiResult {
    auth(&headers, &s)?;
    Ok(Json(
        json!({"ok":true,"stage":"planning","version":env!("CARGO_PKG_VERSION"),"repository":s.repository.display().to_string(),"git":git_state(&s.repository).await,"herdr":"enabled"}),
    ))
}
/// Browse the checked-out tree without pulling or changing the repository.
async fn repository_tree(State(s): State<Arc<AppState>>, headers: HeaderMap) -> ApiResult {
    auth(&headers, &s)?;
    let repo = s.repository.clone();
    let snapshot = tokio::task::spawn_blocking(move || repository::analyze(&repo))
        .await
        .map_err(|e| err(StatusCode::INTERNAL_SERVER_ERROR, e))?
        .map_err(|e| err(StatusCode::UNPROCESSABLE_ENTITY, e))?;
    Ok(Json(
        serde_json::to_value(snapshot).map_err(|e| err(StatusCode::INTERNAL_SERVER_ERROR, e))?,
    ))
}

#[cfg(test)]
mod tests;
