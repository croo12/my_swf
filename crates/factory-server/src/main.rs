mod graph;

use std::{collections::{HashMap, HashSet}, env, path::{Path, PathBuf}, sync::Arc, time::{SystemTime, UNIX_EPOCH, Duration}};
use axum::{
    extract::{Path as RoutePath, State, DefaultBodyLimit},
    http::{HeaderMap, StatusCode, header::AUTHORIZATION},
    routing::{get, post}, Json, Router,
};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use tokio::{fs, net::TcpListener, process::Command, sync::Mutex};
use uuid::Uuid;

// All meaningful routes require an explicit API bearer token. Bind to loopback and use Tailscale Serve.
type ApiError = (StatusCode, String);
type ApiResult = Result<Json<Value>, ApiError>;

#[derive(Clone)]
struct AppState {
    repository: PathBuf,
    data_dir: PathBuf,
    herdr_bin: String,
    api_token: String,
    guard: Arc<Mutex<()>>,
    jobs: Arc<Mutex<HashMap<String, Job>>>,
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Job {
    id: String,
    status: String,
    message: String,
    agent: Option<String>,
    pane_id: Option<String>,
    created_at_unix_ms: u128,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct StartJob {
    instruction: String,
    current_plan: Value,
}

fn err(status: StatusCode, msg: impl ToString) -> ApiError { (status, msg.to_string()) }
fn auth(headers: &HeaderMap, state: &AppState) -> Result<(), ApiError> {
    let supplied = headers.get(AUTHORIZATION).and_then(|x| x.to_str().ok()).and_then(|x| x.strip_prefix("Bearer "));
    if supplied != Some(state.api_token.as_str()) { return Err(err(StatusCode::UNAUTHORIZED, "Invalid factory API token")); }
    Ok(())
}
fn validate_plan(plan: &Value) -> Result<(), ApiError> {
    let nodes = plan.get("nodes").and_then(Value::as_array).ok_or_else(||err(StatusCode::BAD_REQUEST,"nodes array required"))?;
    let edges = plan.get("edges").and_then(Value::as_array).ok_or_else(||err(StatusCode::BAD_REQUEST,"edges array required"))?;
    if nodes.len()>1000 || edges.len()>5000 {return Err(err(StatusCode::BAD_REQUEST,"Graph too large"));}
    let mut ids=HashSet::new();
    for n in nodes {
        let id=n.get("id").and_then(Value::as_str).ok_or_else(||err(StatusCode::BAD_REQUEST,"node id required"))?;
        let path=n.get("path").and_then(Value::as_str).ok_or_else(||err(StatusCode::BAD_REQUEST,"node path required"))?;
        let parts: Vec<_>=path.split('/').collect();
        let valid=match parts.first().copied() {
            Some("app"|"shared")=>parts.len()==2,
            Some("pages"|"widgets"|"features"|"entities")=>parts.len()>=3,
            _=>false,
        };
        if !valid || parts.iter().any(|p|p.is_empty()||*p=="..") {return Err(err(StatusCode::BAD_REQUEST,format!("Invalid FSD path: {path}")));}
        if !ids.insert(id) {return Err(err(StatusCode::BAD_REQUEST,format!("Duplicate id: {id}")));}
        for field in ["inputs","outputs"] {
            if !n.get(field).and_then(Value::as_array).is_some_and(|a| a.iter().all(Value::is_string)) {
                return Err(err(StatusCode::BAD_REQUEST,format!("node {id}: {field} must be string array")));
            }
        }
    }
    for e in edges {
        if !ids.contains(e.get("from").and_then(Value::as_str).unwrap_or("")) ||
            !ids.contains(e.get("to").and_then(Value::as_str).unwrap_or("")) {
            return Err(err(StatusCode::BAD_REQUEST,"Edge references unknown node"));
        }
    }
    Ok(())
}
async fn write_json(path:&Path, value:&Value)->Result<(), ApiError> {
    let bytes=serde_json::to_vec_pretty(value).map_err(|e|err(StatusCode::INTERNAL_SERVER_ERROR,e))?;
    fs::write(path,bytes).await.map_err(|e|err(StatusCode::INTERNAL_SERVER_ERROR,e))
}
async fn read_json(path:&Path)->Result<Value, ApiError> {
    let bytes=fs::read(path).await.map_err(|e|err(StatusCode::NOT_FOUND,e))?;
    serde_json::from_slice(&bytes).map_err(|e|err(StatusCode::INTERNAL_SERVER_ERROR,e))
}
async fn status(State(s):State<Arc<AppState>>,headers:HeaderMap)->ApiResult {
    auth(&headers,&s)?;
    Ok(Json(json!({"ok":true,"stage":"planning","version":"0.2","repository":s.repository.display().to_string(),"herdr":"enabled"})))
}
async fn get_plan(State(s):State<Arc<AppState>>,headers:HeaderMap)->ApiResult {
    auth(&headers,&s)?;
    let _lock=s.guard.lock().await;
    let plan=read_json(&s.data_dir.join("plan.json")).await?;
    Ok(Json(json!({"plan":plan})))
}
async fn put_plan(State(s):State<Arc<AppState>>,headers:HeaderMap,Json(plan):Json<Value>)->ApiResult {
    auth(&headers,&s)?;
    validate_plan(&plan)?;
    let _lock=s.guard.lock().await;
    let tmp=s.data_dir.join("plan.tmp");
    write_json(&tmp,&plan).await?;
    fs::rename(&tmp,s.data_dir.join("plan.json")).await.map_err(|e|err(StatusCode::INTERNAL_SERVER_ERROR,e))?;
    Ok(Json(json!({"saved":true})))
}
async fn approve(State(s):State<Arc<AppState>>,headers:HeaderMap)->ApiResult {
    auth(&headers,&s)?;
    let _lock=s.guard.lock().await;
    let plan=read_json(&s.data_dir.join("plan.json")).await?;
    validate_plan(&plan)?;
    let stamp=now_ms();
    let name=format!("approved/plan-{stamp}.json");
    write_json(&s.data_dir.join(&name),&json!({"approvedAtUnixMs":stamp.to_string(),"plan":plan})).await?;
    Ok(Json(json!({"approved":true,"file":name})))
}
async fn analyze(State(s):State<Arc<AppState>>,headers:HeaderMap)->ApiResult {
    auth(&headers,&s)?;
    let repo=s.repository.clone();
    let result=tokio::task::spawn_blocking(move||graph::scan(&repo)).await.map_err(|e|err(StatusCode::INTERNAL_SERVER_ERROR,e))?;
    let plan=result.map_err(|e|err(StatusCode::UNPROCESSABLE_ENTITY,e))?;
    Ok(Json(json!({"plan":plan})))
}
fn now_ms()->u128 {SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_millis()}
async fn persist_job(s:&AppState,job:&Job) {
    let dir=s.data_dir.join("jobs").join(&job.id);
    let _=fs::create_dir_all(&dir).await;
    if let Ok(v)=serde_json::to_value(job) {let _=write_json(&dir.join("job.json"),&v).await;}
}
async fn update_job(s:&AppState,id:&str,status:&str,message:String,agent:Option<String>,pane_id:Option<String>) {
    let mut jobs=s.jobs.lock().await;
    if let Some(j)=jobs.get_mut(id) {
        j.status=status.into();j.message=message;j.agent=agent.or(j.agent.take());j.pane_id=pane_id.or(j.pane_id.take());
        let snapshot=j.clone();drop(jobs);persist_job(s,&snapshot).await;
    }
}
async fn cli(s:&AppState,args:&[&str],max_wait:Duration)->Result<String,String> {
    let child=Command::new(&s.herdr_bin).args(args).current_dir(&s.repository).kill_on_drop(true).output();
    let output=tokio::time::timeout(max_wait,child).await.map_err(|_|format!("Herdr timeout: {}",args.join(" ")))?
        .map_err(|e|format!("Cannot run Herdr: {e}"))?;
    if !output.status.success() {
        return Err(format!("Herdr exited {:?}: {}",output.status.code(),String::from_utf8_lossy(&output.stderr).chars().take(1500).collect::<String>()));
    }
    Ok(String::from_utf8_lossy(&output.stdout).to_string())
}
async fn execute_job(s:Arc<AppState>,id:String,instruction:String) {
    let agent=format!("factory_{}",&id[..12]);
    let dir=s.data_dir.join("jobs").join(&id);
    let output=dir.join("proposed-plan.json");
    let source_repo=s.repository.to_string_lossy().to_string();
    // A detached worktree ensures the agent cannot modify the active checkout.
    let worktree=dir.join("worktree");
    let repo=worktree.to_string_lossy().to_string();
    let input=worktree.join(".factory/input-plan.json");
    let generated=worktree.join(".factory/proposed-plan.json");
    let dir_name=format!("factory-{}",&id[..8]);
    let result:Result<(String,String),String>=async {
        update_job(&s,&id,"running","격리된 Git worktree 생성 중".into(),None,None).await;
        let outcome=Command::new("git").args(["-C",&source_repo,"worktree","add","--detach",&repo,"HEAD"])
            .output().await.map_err(|e|format!("git worktree failed: {e}"))?;
        if !outcome.status.success() {return Err(format!("git worktree failed: {}",String::from_utf8_lossy(&outcome.stderr)));}
        fs::create_dir_all(worktree.join(".factory")).await.map_err(|e|e.to_string())?;
        fs::copy(dir.join("input-plan.json"),&input).await.map_err(|e|e.to_string())?;
        update_job(&s,&id,"running","Herdr workspace 생성 중".into(),None,None).await;
        let result=cli(&s,&["workspace","create","--cwd",&repo,"--label",&dir_name,"--no-focus"],Duration::from_secs(35)).await?;
        let data:Value=serde_json::from_str(&result).map_err(|e|format!("Workspace JSON decode failed: {e}: {result}"))?;
        let pane=data.pointer("/result/root_pane/pane_id").and_then(Value::as_str).ok_or_else(||format!("Herdr root pane id missing: {result}"))?.to_owned();
        update_job(&s,&id,"running","Codex agent 시작 중".into(),Some(agent.clone()),Some(pane.clone())).await;
        cli(&s,&["agent","start",&agent,"--kind","codex","--pane",&pane],Duration::from_secs(65)).await?;
        let prompt=format!("You are the planning stage of a software factory. Work only in planning mode; do not modify production repository files. Inspect the repo at {repo}. Read the user's input in {} (JSON with current plan and instruction). User instruction: {}. Output a complete proposed Planning Workbench graph in JSON at {}. Required schema: {{\"schemaVersion\":\"0.3\",\"version\":1,\"nodes\":[{{\"id\":\"...\",\"path\":\"features/example/model\",\"description\":\"...\",\"inputs\":[\"...\"],\"outputs\":[\"...\"],\"x\":210,\"y\":150}}],\"edges\":[{{\"id\":\"...\",\"from\":\"node-id\",\"to\":\"node-id\",\"type\":\"imports\"}}],\"requests\":[]}}. Use FSD layers app/pages/widgets/features/entities/shared and actual segments. App/shared have two-part paths, others three-part paths. Keep existing user design changes where possible. Produce a COMPLETE JSON, no markdown. If uncertain label inferred details in description. Final response should mention the output path only.",input.display(),instruction,output.display());
        update_job(&s,&id,"running","Codex에서 계획 생성 중".into(),Some(agent.clone()),Some(pane.clone())).await;
        cli(&s,&["agent","prompt",&agent,&prompt,"--wait","--timeout","600000"],Duration::from_secs(630)).await?;
        fs::copy(&generated,&output).await.map_err(|e|format!("No plan JSON was written inside the worktree: {e}"))?;
        Ok((agent,pane))
    }.await;
    match result {
        Ok((agent,pane)) => {
            let parsed=read_json(&output).await.map_err(|e|e.1);
            match parsed {
                Ok(plan) => match validate_plan(&plan) {
                    Ok(()) => update_job(&s,&id,"completed","계획 제안 준비됨. UI에서 확인 후 적용하세요.".into(),Some(agent),Some(pane)).await,
                    Err(e) => update_job(&s,&id,"failed",format!("AI JSON validation failed: {}",e.1),Some(agent),Some(pane)).await,
                },
                Err(e) => update_job(&s,&id,"failed",format!("AI did not produce a valid plan file: {e}"),Some(agent),Some(pane)).await,
            }
        }
        Err(error) => update_job(&s,&id,"failed",error,None,None).await,
    }
}
async fn start_job(State(s):State<Arc<AppState>>,headers:HeaderMap,Json(input):Json<StartJob>)->ApiResult {
    auth(&headers,&s)?;
    validate_plan(&input.current_plan)?;
    let instruction=input.instruction.trim();
    if instruction.is_empty() || instruction.chars().count()>3000 {return Err(err(StatusCode::BAD_REQUEST,"Instruction must be 1..3000 chars"));}
    let id=Uuid::new_v4().simple().to_string();
    let job=Job{id:id.clone(),status:"queued".into(),message:"Herdr 작업 대기".into(),agent:None,pane_id:None,created_at_unix_ms:now_ms()};
    let dir=s.data_dir.join("jobs").join(&id);
    fs::create_dir_all(&dir).await.map_err(|e|err(StatusCode::INTERNAL_SERVER_ERROR,e))?;
    write_json(&dir.join("input-plan.json"),&json!({"instruction":instruction,"plan":input.current_plan})).await?;
    persist_job(&s,&job).await;
    s.jobs.lock().await.insert(id.clone(),job);
    let cloned=Arc::clone(&s);
    let request=instruction.to_owned();
    let job_id=id.clone();
    tokio::spawn(async move { execute_job(cloned,job_id,request).await; });
    Ok(Json(json!({"id":id,"status":"queued"})))
}
async fn job_status(State(s):State<Arc<AppState>>,headers:HeaderMap,RoutePath(id):RoutePath<String>)->ApiResult {
    auth(&headers,&s)?;
    if id.len()!=32 || !id.chars().all(|c|c.is_ascii_hexdigit()) {return Err(err(StatusCode::BAD_REQUEST,"Invalid job id"));}
    let mut job=s.jobs.lock().await.get(&id).cloned();
    if job.is_none() {
        let file=s.data_dir.join("jobs").join(&id).join("job.json");
        if let Ok(v)=read_json(&file).await {
            if let Ok(mut j)=serde_json::from_value::<Job>(v) {
                if j.status=="running" || j.status=="queued" {j.status="interrupted".into();j.message="Factory 서버 재시작으로 작업 추적이 중단되었습니다. Herdr pane을 직접 확인하세요.".into();}
                job=Some(j);
            }
        }
    }
    let job=job.ok_or_else(||err(StatusCode::NOT_FOUND,"Job not found"))?;
    let plan=if job.status=="completed" {
        Some(read_json(&s.data_dir.join("jobs").join(&id).join("proposed-plan.json")).await?)
    } else { None };
    Ok(Json(json!({"job":job,"proposedPlan":plan})))
}

#[tokio::main]
async fn main()->Result<(),Box<dyn std::error::Error>> {
    let token=env::var("FACTORY_API_TOKEN").map_err(|_|"Set FACTORY_API_TOKEN (a long random secret)")?;
    if token.len()<24 {return Err("FACTORY_API_TOKEN must be at least 24 characters".into());}
    let repo=std::path::absolute(env::var("FACTORY_REPO_PATH").map_err(|_|"Set FACTORY_REPO_PATH")?)?;
    if !repo.is_dir() {return Err("FACTORY_REPO_PATH must be a directory".into());}
    let data_dir=PathBuf::from(env::var("FACTORY_DATA_DIR").unwrap_or_else(|_|"./data".into()));
    fs::create_dir_all(data_dir.join("approved")).await?;
    fs::create_dir_all(data_dir.join("jobs")).await?;
    // std::path::absolute, not canonicalize: on Windows canonicalize yields \\?\ verbatim paths that git and other CLIs reject.
    let data_dir=std::path::absolute(data_dir)?;
    let state=Arc::new(AppState{repository:repo,data_dir,herdr_bin:env::var("HERDR_BIN").unwrap_or_else(|_|"herdr".into()),api_token:token,guard:Arc::new(Mutex::new(())),jobs:Arc::new(Mutex::new(HashMap::new()))});
    let app=Router::new()
        .route("/api/status",get(status))
        .route("/api/plan",get(get_plan).put(put_plan))
        .route("/api/approve",post(approve))
        .route("/api/repository/analyze",post(analyze))
        .route("/api/jobs",post(start_job))
        .route("/api/jobs/{id}",get(job_status))
        .layer(DefaultBodyLimit::max(4*1024*1024))
        .with_state(state);
    let addr=env::var("FACTORY_BIND").unwrap_or_else(|_|"127.0.0.1:8787".into());
    let listener=TcpListener::bind(&addr).await?;
    println!("Factory server bound to {addr}; expose through Tailscale Serve, not Funnel.");
    axum::serve(listener,app).await?;
    Ok(())
}
