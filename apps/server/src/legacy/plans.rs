use super::{
    planner,
    repository::scan_repository,
    storage::{now_ms, read_json, write_json},
};
use crate::git::git_state;
use crate::state::{auth, err, ApiError, ApiResult, AppState};
use axum::{
    extract::State,
    http::{HeaderMap, StatusCode},
    Json,
};
use serde_json::{json, Value};
use std::{collections::HashSet, sync::Arc};
use tokio::fs;

pub(crate) fn validate_plan(plan: &Value) -> Result<(), ApiError> {
    let nodes = plan
        .get("nodes")
        .and_then(Value::as_array)
        .ok_or_else(|| err(StatusCode::BAD_REQUEST, "nodes array required"))?;
    let edges = plan
        .get("edges")
        .and_then(Value::as_array)
        .ok_or_else(|| err(StatusCode::BAD_REQUEST, "edges array required"))?;
    if nodes.len() > 1000 || edges.len() > 5000 {
        return Err(err(StatusCode::BAD_REQUEST, "Graph too large"));
    }
    let mut ids = HashSet::new();
    for n in nodes {
        let id = n
            .get("id")
            .and_then(Value::as_str)
            .ok_or_else(|| err(StatusCode::BAD_REQUEST, "node id required"))?;
        let path = n
            .get("path")
            .and_then(Value::as_str)
            .ok_or_else(|| err(StatusCode::BAD_REQUEST, "node path required"))?;
        let parts: Vec<_> = path.split('/').collect();
        let valid = match parts.first().copied() {
            Some("app" | "shared") => parts.len() == 2,
            Some("pages" | "widgets" | "features" | "entities") => parts.len() >= 3,
            _ => false,
        };
        if !valid || parts.iter().any(|p| p.is_empty() || *p == "..") {
            return Err(err(
                StatusCode::BAD_REQUEST,
                format!("Invalid FSD path: {path}"),
            ));
        }
        if !ids.insert(id) {
            return Err(err(StatusCode::BAD_REQUEST, format!("Duplicate id: {id}")));
        }
        // Interfaces carry their own inputs and outputs; plans saved by older clients may still
        // have them on the node. Either way, where present they must be lists of strings.
        let string_list = |v: Option<&Value>| {
            v.is_none_or(|v| v.as_array().is_some_and(|a| a.iter().all(Value::is_string)))
        };
        if !string_list(n.get("inputs")) || !string_list(n.get("outputs")) {
            return Err(err(
                StatusCode::BAD_REQUEST,
                format!("node {id}: inputs and outputs must be string arrays"),
            ));
        }
        for interface in n
            .get("interfaces")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
        {
            if interface
                .get("name")
                .and_then(Value::as_str)
                .is_none_or(|name| name.trim().is_empty())
                || !string_list(interface.get("inputs"))
                || !string_list(interface.get("outputs"))
            {
                return Err(err(StatusCode::BAD_REQUEST,format!("node {id}: every interface needs a name and string arrays for inputs and outputs")));
            }
        }
    }
    for e in edges {
        if e.get("uses")
            .is_some_and(|u| !u.as_array().is_some_and(|a| a.iter().all(Value::is_string)))
        {
            return Err(err(
                StatusCode::BAD_REQUEST,
                "Edge uses must be a list of interface names",
            ));
        }
        if !ids.contains(e.get("from").and_then(Value::as_str).unwrap_or(""))
            || !ids.contains(e.get("to").and_then(Value::as_str).unwrap_or(""))
        {
            return Err(err(StatusCode::BAD_REQUEST, "Edge references unknown node"));
        }
    }
    Ok(())
}
pub(crate) async fn get_plan(State(s): State<Arc<AppState>>, headers: HeaderMap) -> ApiResult {
    auth(&headers, &s)?;
    let _lock = s.guard.lock().await;
    let plan = read_json(&s.data_dir.join("plan.json")).await?;
    Ok(Json(json!({"plan":plan})))
}
pub(crate) async fn put_plan(
    State(s): State<Arc<AppState>>,
    headers: HeaderMap,
    Json(plan): Json<Value>,
) -> ApiResult {
    auth(&headers, &s)?;
    validate_plan(&plan)?;
    let _lock = s.guard.lock().await;
    let tmp = s.data_dir.join("plan.tmp");
    write_json(&tmp, &plan).await?;
    fs::rename(&tmp, s.data_dir.join("plan.json"))
        .await
        .map_err(|e| err(StatusCode::INTERNAL_SERVER_ERROR, e))?;
    Ok(Json(json!({"saved":true})))
}
pub(crate) async fn approve(State(s): State<Arc<AppState>>, headers: HeaderMap) -> ApiResult {
    auth(&headers, &s)?;
    let _lock = s.guard.lock().await;
    let plan = read_json(&s.data_dir.join("plan.json")).await?;
    validate_plan(&plan)?;
    let stamp = now_ms();
    let name = format!("approved/plan-{stamp}.json");
    write_json(
        &s.data_dir.join(&name),
        &json!({"approvedAtUnixMs":stamp.to_string(),"plan":plan}),
    )
    .await?;
    Ok(Json(json!({"approved":true,"file":name})))
}
/// Checks a plan drawn by a person with the rules used for proposals, and returns the code around
/// it. Nothing is stored.
pub(crate) async fn check_plan(
    State(s): State<Arc<AppState>>,
    headers: HeaderMap,
    Json(input): Json<Value>,
) -> ApiResult {
    auth(&headers, &s)?;
    let plan = input.get("plan").cloned().unwrap_or(Value::Null);
    let scan = scan_repository(&s).await?;
    let git = git_state(&s.repository).await;
    let proposal = match planner::proposal_from_plan(&plan) {
        Ok(proposal) => proposal,
        Err(reason) => {
            return Ok(Json(
                json!({"errors":[reason],"nodes":[],"edges":[],"git":git}),
            ))
        }
    };
    let paths: HashSet<String> = scan
        .get("nodes")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|n| n.get("path").and_then(Value::as_str).map(str::to_owned))
        .collect();
    let root = s.repository.join(
        scan.pointer("/analysis/sourceRoot")
            .and_then(Value::as_str)
            .unwrap_or(""),
    );
    let errors = planner::validate(
        &proposal,
        &planner::Disk {
            paths: &paths,
            file_exists: &|file| root.join(file).is_file(),
        },
        planner::Author::Person,
    );
    let (nodes, edges) = planner::surroundings(&proposal, &planner::observed_imports(&scan));
    Ok(Json(
        json!({"errors":errors,"nodes":nodes,"edges":edges,"git":git}),
    ))
}
