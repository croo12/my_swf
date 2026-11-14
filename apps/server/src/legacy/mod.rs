//! Compatibility API for the retired AI planning client.
//! The native app only uses the read-only repository route in crate::api.
pub(crate) mod agent;
pub(crate) mod jobs;
pub(crate) mod planner;
mod plans;
mod prompt;
mod repository;
mod storage;

use crate::state::AppState;
use axum::{
    routing::{get, post},
    Router,
};
use std::sync::Arc;

pub(crate) fn router() -> Router<Arc<AppState>> {
    Router::new()
        .route("/api/plan", get(plans::get_plan).put(plans::put_plan))
        .route("/api/approve", post(plans::approve))
        .route("/api/repository/analyze", post(repository::analyze))
        .route("/api/repository/modules", post(repository::slice_modules))
        .route("/api/plan/check", post(plans::check_plan))
        .route("/api/jobs", post(jobs::start_job))
        .route("/api/jobs/{id}", get(jobs::job_status))
}
