use crate::legacy::jobs::Job;
use axum::{
    http::{header::AUTHORIZATION, HeaderMap, StatusCode},
    Json,
};
use serde_json::Value;
use std::{collections::HashMap, path::PathBuf, sync::Arc};
use tokio::sync::Mutex;

pub(crate) type ApiError = (StatusCode, String);
pub(crate) type ApiResult = Result<Json<Value>, ApiError>;

#[derive(Clone)]
pub(crate) struct AppState {
    pub(crate) repository: PathBuf,
    pub(crate) data_dir: PathBuf,
    pub(crate) herdr_bin: String,
    pub(crate) api_token: String,
    pub(crate) guard: Arc<Mutex<()>>,
    pub(crate) jobs: Arc<Mutex<HashMap<String, Job>>>,
}

pub(crate) fn err(status: StatusCode, msg: impl ToString) -> ApiError {
    (status, msg.to_string())
}
pub(crate) fn auth(headers: &HeaderMap, state: &AppState) -> Result<(), ApiError> {
    let supplied = headers
        .get(AUTHORIZATION)
        .and_then(|x| x.to_str().ok())
        .and_then(|x| x.strip_prefix("Bearer "));
    if supplied != Some(state.api_token.as_str()) {
        return Err(err(StatusCode::UNAUTHORIZED, "Invalid factory API token"));
    }
    Ok(())
}
