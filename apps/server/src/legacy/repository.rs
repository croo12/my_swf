use crate::git::{git_state, pull_latest};
use crate::state::{auth, err, ApiResult, AppState};
use axum::{
    extract::State,
    http::{HeaderMap, StatusCode},
    Json,
};
use factory_repository::graph;
use serde_json::{json, Value};
use std::sync::Arc;

pub(crate) async fn analyze(State(s): State<Arc<AppState>>, headers: HeaderMap) -> ApiResult {
    auth(&headers, &s)?;
    let pull_error = pull_latest(&s.repository).await.err();
    let repo = s.repository.clone();
    let result = tokio::task::spawn_blocking(move || graph::scan(&repo))
        .await
        .map_err(|e| err(StatusCode::INTERNAL_SERVER_ERROR, e))?;
    let mut plan = result.map_err(|e| err(StatusCode::UNPROCESSABLE_ENTITY, e))?;
    // The per-import records are for building plans; the overview only needs which node imports which.
    if let Some(fields) = plan.as_object_mut() {
        fields.remove("records");
    }
    Ok(Json(
        json!({"plan":plan,"git":git_state(&s.repository).await,"pullError":pull_error}),
    ))
}
/// Scans the repository as it is now. Plans are checked against the checkout the server has; the
/// pull happens when the repository is analyzed or a job starts.
pub(crate) async fn scan_repository(s: &AppState) -> Result<Value, (StatusCode, String)> {
    let repo = s.repository.clone();
    tokio::task::spawn_blocking(move || graph::scan(&repo))
        .await
        .map_err(|e| err(StatusCode::INTERNAL_SERVER_ERROR, e))?
        .map_err(|e| err(StatusCode::UNPROCESSABLE_ENTITY, e))
}
/// The modules of one slice as they exist in the repository, in the shape of plan modules.
pub(crate) async fn slice_modules(
    State(s): State<Arc<AppState>>,
    headers: HeaderMap,
    Json(input): Json<Value>,
) -> ApiResult {
    auth(&headers, &s)?;
    let key = input
        .get("slice")
        .and_then(Value::as_str)
        .unwrap_or("")
        .trim()
        .trim_matches('/')
        .to_owned();
    if key.is_empty() {
        return Err(err(StatusCode::BAD_REQUEST, "slice is required"));
    }
    let scan = scan_repository(&s).await?;
    let repo = s.repository.clone();
    let modules = tokio::task::spawn_blocking(move || graph::slice_modules(&repo, &scan, &key))
        .await
        .map_err(|e| err(StatusCode::INTERNAL_SERVER_ERROR, e))?
        .map_err(|e| err(StatusCode::UNPROCESSABLE_ENTITY, e))?;
    Ok(Json(modules))
}
