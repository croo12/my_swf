mod graph;
mod planner;

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
        // Interfaces carry their own inputs and outputs; plans saved by older clients may still
        // have them on the node. Either way, where present they must be lists of strings.
        let string_list=|v:Option<&Value>|v.is_none_or(|v|v.as_array().is_some_and(|a|a.iter().all(Value::is_string)));
        if !string_list(n.get("inputs")) || !string_list(n.get("outputs")) {
            return Err(err(StatusCode::BAD_REQUEST,format!("node {id}: inputs and outputs must be string arrays")));
        }
        for interface in n.get("interfaces").and_then(Value::as_array).into_iter().flatten() {
            if interface.get("name").and_then(Value::as_str).is_none_or(|name|name.trim().is_empty()) || !string_list(interface.get("inputs")) || !string_list(interface.get("outputs")) {
                return Err(err(StatusCode::BAD_REQUEST,format!("node {id}: every interface needs a name and string arrays for inputs and outputs")));
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
/// The task for the agent. It describes what to plan and gives it the facts it needs. The shape of
/// the answer is not requested here: it is enforced by the output schema, and the rules are
/// enforced by `planner::validate`, which rejects the answer if any of them is broken.
fn planning_prompt(repo:&str,source_root:&str,instruction:&str,current_plan:&Value,existing_paths:&[String])->String {
    format!(r#"Plan work for a codebase that follows Feature-Sliced Design. Do not change any file.

Repository: {repo}
FSD source root (relative to the repository): {source_root}

Request: {instruction}

Read the code the request touches. Then answer with the modules to build or change: what each one is responsible for, what it does, its interfaces (what it offers to other modules and the requests it sends to the server, each with its kind and with inputs and outputs where it has any), where it is placed (layer, slice, segment), and which modules it imports from. A module is a unit with one responsibility that a developer can implement and review on its own. Write names and descriptions in the language of the request.

Your answer is checked by a program and rejected as a whole if any of these is not true:
- Every module has a name, a responsibility, and a placement in one of the six FSD layers. pages, widgets, features and entities need a slice; app and shared have none.
- A module marked "modify" or "existing" is at a placement that exists in the repository (list below), and every file it names exists there. "existing" modules are included only when another module in the plan imports from them.
- Every module names its files. A "new" module names files that do not exist yet. Only client source files (ts, tsx, js, jsx, css, scss, json) can be planned; code outside the FSD layers is not part of a plan.
- Every "new" or "modify" module states at least one behavior and has at least one interface. An interface has a unique name and a kind; its inputs and outputs may be empty.
- A module offers something other than types to other modules: a component, hook, function, store or constant. A file that only declares types is not a module; list its types and its file in the module that uses them.
- A request to the server is an interface of kind "server", named METHOD /path. A module with such an interface is placed in shared/api or depends, directly or indirectly, on a module in shared/api.
- A dependency points to a lower layer, or stays inside one slice. There are no cycles, no duplicates, and no dependencies that start at an "existing" module.

The plan so far (keep these modules unless the request requires changing them):
{current_plan}

Module placements that exist in the repository:
{}"#,existing_paths.join("\n"))
}

/// Runs the agent in the Herdr pane so its progress can be watched, by starting this same binary
/// as `factory-server run-codex <job-dir>`. Going through our own binary keeps the command line
/// free of quoting, which differs between the shells a pane may run.
async fn run_agent_in_pane(s:&AppState,dir:&Path,pane:&str)->Result<(),String> {
    let exe=env::current_exe().map_err(|e|format!("Cannot locate the server binary: {e}"))?;
    let (exe,job_dir)=(exe.to_string_lossy().to_string(),dir.to_string_lossy().to_string());
    if exe.contains(char::is_whitespace) || job_dir.contains(char::is_whitespace) {
        return Err(format!("Paths with spaces cannot be passed to the Herdr pane: {exe} / {job_dir}"));
    }
    cli(s,&["pane","run",pane,&exe,"run-codex",&job_dir],Duration::from_secs(35)).await?;
    Ok(())
}

/// Entry point of `factory-server run-codex <job-dir>`. Runs `codex exec` with the job's prompt on
/// stdin, constrained to the job's output schema and to a read-only sandbox, and records the exit
/// code in exit.json for the server to pick up.
fn run_codex(job_dir:&Path)->i32 {
    let run=|| -> Result<i32,String> {
        let settings:Value=serde_json::from_slice(&std::fs::read(job_dir.join("run.json")).map_err(|e|format!("run.json: {e}"))?).map_err(|e|format!("run.json: {e}"))?;
        let cwd=settings.get("cwd").and_then(Value::as_str).ok_or("run.json has no cwd")?;
        let codex=env::var("CODEX_BIN").unwrap_or_else(|_|"codex".into());
        let prompt=std::fs::File::open(job_dir.join("prompt.txt")).map_err(|e|format!("prompt.txt: {e}"))?;
        // On Windows codex is a .cmd shim, which only the command interpreter can start.
        let mut command=if cfg!(windows) {let mut c=std::process::Command::new("cmd");c.args(["/C",&codex]);c} else {std::process::Command::new(&codex)};
        let status=command.arg("exec")
            .args(["--sandbox","read-only","--ephemeral","--skip-git-repo-check","-c","check_for_update_on_startup=false","-C",cwd])
            .arg("--output-schema").arg(job_dir.join("schema.json"))
            .arg("--output-last-message").arg(job_dir.join("result.json"))
            .arg("-")
            .stdin(prompt)
            .status().map_err(|e|format!("Cannot start {codex}: {e}"))?;
        Ok(status.code().unwrap_or(-1))
    };
    let (code,error)=match run() {Ok(code)=>(code,None),Err(e)=>{eprintln!("{e}");(-1,Some(e))}};
    let _=std::fs::write(job_dir.join("exit.json"),json!({"code":code,"error":error}).to_string());
    code
}

const AGENT_TIMEOUT:Duration=Duration::from_secs(900);

async fn execute_job(s:Arc<AppState>,id:String,instruction:String) {
    let dir=s.data_dir.join("jobs").join(&id);
    let label=format!("factory-{}",&id[..8]);
    let result:Result<Value,String>=async {
        // 1. Facts from the repository: where the FSD layers are and which modules exist.
        update_job(&s,&id,"running","저장소 구조 확인 중".into(),None,None).await;
        let repo=s.repository.clone();
        let scan=tokio::task::spawn_blocking(move||graph::scan(&repo)).await.map_err(|e|e.to_string())??;
        let existing:Vec<String>=scan.get("nodes").and_then(Value::as_array).into_iter().flatten()
            .filter_map(|n|n.get("path").and_then(Value::as_str).map(str::to_owned)).collect();
        let source_root=scan.pointer("/analysis/sourceRoot").and_then(Value::as_str).unwrap_or("").to_owned();
        let input=read_json(&dir.join("input-plan.json")).await.map_err(|e|e.1)?;
        let current_plan=input.get("plan").cloned().unwrap_or(Value::Null);

        // 2. The job files: task, answer schema, and where the agent runs.
        let prompt=planning_prompt(&s.repository.to_string_lossy(),&source_root,&instruction,&current_plan,&existing);
        fs::write(dir.join("prompt.txt"),prompt).await.map_err(|e|e.to_string())?;
        write_json(&dir.join("schema.json"),&planner::output_schema()).await.map_err(|e|e.1)?;
        write_json(&dir.join("run.json"),&json!({"cwd":s.repository})).await.map_err(|e|e.1)?;

        // 3. Run the agent in a Herdr pane and wait for it to finish.
        update_job(&s,&id,"running","Herdr workspace 생성 중".into(),None,None).await;
        let created=cli(&s,&["workspace","create","--cwd",&s.repository.to_string_lossy(),"--label",&label,"--no-focus"],Duration::from_secs(35)).await?;
        let data:Value=serde_json::from_str(&created).map_err(|e|format!("Workspace JSON decode failed: {e}: {created}"))?;
        let pane=data.pointer("/result/root_pane/pane_id").and_then(Value::as_str).ok_or_else(||format!("Herdr root pane id missing: {created}"))?.to_owned();
        // A new pane needs a moment before its shell accepts a command.
        tokio::time::sleep(Duration::from_secs(3)).await;
        run_agent_in_pane(&s,&dir,&pane).await?;
        update_job(&s,&id,"running",format!("Codex가 계획을 만드는 중 (Herdr {label})"),Some("codex".into()),Some(pane)).await;
        let exit_file=dir.join("exit.json");
        let started=std::time::Instant::now();
        while !exit_file.is_file() {
            if started.elapsed()>AGENT_TIMEOUT {return Err(format!("Codex did not finish within {} minutes.",AGENT_TIMEOUT.as_secs()/60));}
            tokio::time::sleep(Duration::from_secs(2)).await;
        }
        let exit=read_json(&exit_file).await.map_err(|e|e.1)?;
        if exit.get("code").and_then(Value::as_i64)!=Some(0) {
            return Err(format!("Codex exited with an error: {}. See the Herdr workspace {label}.",exit.get("error").and_then(Value::as_str).unwrap_or("non-zero exit code")));
        }

        // 4. The answer must parse into the typed proposal and pass every static check.
        update_job(&s,&id,"running","제안 검증 중".into(),None,None).await;
        let answer=fs::read_to_string(dir.join("result.json")).await.map_err(|e|format!("Codex produced no answer: {e}"))?;
        let proposal:planner::Proposal=serde_json::from_str(&answer).map_err(|e|format!("제안이 정해진 구조가 아니어서 거절했습니다: {e}"))?;
        let paths:HashSet<String>=existing.iter().cloned().collect();
        let root=s.repository.join(&source_root);
        let errors=planner::validate(&proposal,&planner::Disk{paths:&paths,file_exists:&|file|root.join(file).is_file()});
        if !errors.is_empty() {
            let _=write_json(&dir.join("rejected.json"),&json!({"errors":errors})).await;
            return Err(format!("제안이 검증을 통과하지 못해 거절했습니다 ({}건):\n- {}",errors.len(),errors.join("\n- ")));
        }
        Ok(planner::to_plan(&proposal))
    }.await;
    match result {
        Ok(plan) => match write_json(&dir.join("proposed-plan.json"),&plan).await {
            Ok(()) => update_job(&s,&id,"completed","계획 제안이 검증을 통과했습니다. 확인 후 적용하세요.".into(),None,None).await,
            Err(e) => update_job(&s,&id,"failed",e.1,None,None).await,
        },
        Err(error) => update_job(&s,&id,"failed",error,None,None).await,
    }
}
async fn start_job(State(s):State<Arc<AppState>>,headers:HeaderMap,Json(input):Json<StartJob>)->ApiResult {
    auth(&headers,&s)?;
    validate_plan(&input.current_plan)?;
    let instruction=input.instruction.trim();
    if instruction.is_empty() || instruction.chars().count()>3000 {return Err(err(StatusCode::BAD_REQUEST,"Instruction must be 1..3000 chars"));}
    let id=Uuid::new_v4().simple().to_string();
    let job=Job{id:id.clone(),status:"queued".into(),message:"계획 작업 대기".into(),agent:None,pane_id:None,created_at_unix_ms:now_ms()};
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
    // `factory-server run-codex <job-dir>` runs the agent for one job; see run_agent_in_pane.
    let args:Vec<String>=env::args().collect();
    if args.get(1).map(String::as_str)==Some("run-codex") {
        let job_dir=args.get(2).ok_or("Usage: factory-server run-codex <job-dir>")?;
        std::process::exit(run_codex(Path::new(job_dir)));
    }
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
