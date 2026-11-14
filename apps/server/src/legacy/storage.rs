use crate::state::{err, ApiError};
use axum::http::StatusCode;
use serde_json::Value;
use std::{
    path::Path,
    time::{SystemTime, UNIX_EPOCH},
};
use tokio::fs;

pub(crate) async fn write_json(path: &Path, value: &Value) -> Result<(), ApiError> {
    let bytes =
        serde_json::to_vec_pretty(value).map_err(|e| err(StatusCode::INTERNAL_SERVER_ERROR, e))?;
    fs::write(path, bytes)
        .await
        .map_err(|e| err(StatusCode::INTERNAL_SERVER_ERROR, e))
}
pub(crate) async fn read_json(path: &Path) -> Result<Value, ApiError> {
    let bytes = fs::read(path)
        .await
        .map_err(|e| err(StatusCode::NOT_FOUND, e))?;
    serde_json::from_slice(&bytes).map_err(|e| err(StatusCode::INTERNAL_SERVER_ERROR, e))
}
pub(crate) fn now_ms() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis()
}
