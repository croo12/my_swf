mod execute;
use super::{
    plans::validate_plan,
    storage::{now_ms, read_json, write_json},
};
use crate::state::{auth, err, ApiResult, AppState};
use axum::{
    extract::{Path as RoutePath, State},
    http::{HeaderMap, StatusCode},
    Json,
};
use execute::execute_job;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::sync::Arc;
use tokio::fs;
use uuid::Uuid;

#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct Job {
    pub(crate) id: String,
    pub(crate) status: String,
    pub(crate) message: String,
    pub(crate) agent: Option<String>,
    pub(crate) pane_id: Option<String>,
    pub(crate) created_at_unix_ms: u128,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct StartJob {
    pub(crate) instruction: String,
    pub(crate) current_plan: Value,
}

pub(crate) async fn persist_job(s: &AppState, job: &Job) {
    let dir = s.data_dir.join("jobs").join(&job.id);
    let _ = fs::create_dir_all(&dir).await;
    if let Ok(v) = serde_json::to_value(job) {
        let _ = write_json(&dir.join("job.json"), &v).await;
    }
}
pub(crate) async fn update_job(
    s: &AppState,
    id: &str,
    status: &str,
    message: String,
    agent: Option<String>,
    pane_id: Option<String>,
) {
    let mut jobs = s.jobs.lock().await;
    if let Some(j) = jobs.get_mut(id) {
        j.status = status.into();
        j.message = message;
        j.agent = agent.or(j.agent.take());
        j.pane_id = pane_id.or(j.pane_id.take());
        let snapshot = j.clone();
        drop(jobs);
        persist_job(s, &snapshot).await;
    }
}
pub(crate) async fn start_job(
    State(s): State<Arc<AppState>>,
    headers: HeaderMap,
    Json(input): Json<StartJob>,
) -> ApiResult {
    auth(&headers, &s)?;
    validate_plan(&input.current_plan)?;
    let instruction = input.instruction.trim();
    if instruction.is_empty() || instruction.chars().count() > 3000 {
        return Err(err(
            StatusCode::BAD_REQUEST,
            "Instruction must be 1..3000 chars",
        ));
    }
    let id = Uuid::new_v4().simple().to_string();
    let job = Job {
        id: id.clone(),
        status: "queued".into(),
        message: "계획 작업 대기".into(),
        agent: None,
        pane_id: None,
        created_at_unix_ms: now_ms(),
    };
    let dir = s.data_dir.join("jobs").join(&id);
    fs::create_dir_all(&dir)
        .await
        .map_err(|e| err(StatusCode::INTERNAL_SERVER_ERROR, e))?;
    write_json(
        &dir.join("input-plan.json"),
        &json!({"instruction":instruction,"plan":input.current_plan}),
    )
    .await?;
    persist_job(&s, &job).await;
    s.jobs.lock().await.insert(id.clone(), job);
    let cloned = Arc::clone(&s);
    let request = instruction.to_owned();
    let job_id = id.clone();
    tokio::spawn(async move {
        execute_job(cloned, job_id, request).await;
    });
    Ok(Json(json!({"id":id,"status":"queued"})))
}
pub(crate) async fn job_status(
    State(s): State<Arc<AppState>>,
    headers: HeaderMap,
    RoutePath(id): RoutePath<String>,
) -> ApiResult {
    auth(&headers, &s)?;
    if id.len() != 32 || !id.chars().all(|c| c.is_ascii_hexdigit()) {
        return Err(err(StatusCode::BAD_REQUEST, "Invalid job id"));
    }
    let mut job = s.jobs.lock().await.get(&id).cloned();
    if job.is_none() {
        let file = s.data_dir.join("jobs").join(&id).join("job.json");
        if let Ok(v) = read_json(&file).await {
            if let Ok(mut j) = serde_json::from_value::<Job>(v) {
                if j.status == "running" || j.status == "queued" {
                    j.status = "interrupted".into();
                    j.message="Factory 서버 재시작으로 작업 추적이 중단되었습니다. Herdr pane을 직접 확인하세요.".into();
                }
                job = Some(j);
            }
        }
    }
    let job = job.ok_or_else(|| err(StatusCode::NOT_FOUND, "Job not found"))?;
    let plan = if job.status == "completed" {
        Some(read_json(&s.data_dir.join("jobs").join(&id).join("proposed-plan.json")).await?)
    } else {
        None
    };
    Ok(Json(json!({"job":job,"proposedPlan":plan})))
}
