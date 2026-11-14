use super::super::{
    agent::{cli, run_agent_in_pane},
    planner,
    prompt::planning_prompt,
    storage::{read_json, write_json},
};
use super::update_job;
use crate::{
    git::{git_state, pull_latest},
    state::AppState,
};
use factory_repository::graph;
use serde_json::{json, Value};
use std::{collections::HashSet, env, sync::Arc, time::Duration};
use tokio::fs;

const AGENT_TIMEOUT: Duration = Duration::from_secs(900);
/// How often the agent is asked: once, plus retries that carry the violations of the previous answer.
const MAX_ATTEMPTS: usize = 3;

pub(crate) async fn execute_job(s: Arc<AppState>, id: String, instruction: String) {
    let dir = s.data_dir.join("jobs").join(&id);
    let label = format!("factory-{}", &id[..8]);
    let result:Result<Value,String>=async {
        // 1. Facts from the repository: where the FSD layers are and which modules exist.
        update_job(&s,&id,"running","저장소를 최신으로 갱신 중".into(),None,None).await;
        let pull_error=pull_latest(&s.repository).await.err();
        let git=git_state(&s.repository).await;
        let based_on=match (git.get("branch").and_then(Value::as_str),git.get("commit").and_then(Value::as_str)) {
            (Some(branch),Some(commit)) => format!("{branch} @ {commit}{}{}",
                if git.get("uncommittedFiles").and_then(Value::as_u64).unwrap_or(0)>0 {" + 미커밋 변경"} else {""},
                // A plan on stale code must say so.
                pull_error.as_ref().map(|reason|format!(" (갱신 실패, 이전 상태 기준: {reason})")).unwrap_or_default()),
            _ => "Git 정보 없음".to_owned(),
        };
        let _=write_json(&dir.join("based-on.json"),&json!({"git":git,"pullError":pull_error})).await;
        update_job(&s,&id,"running",format!("저장소 구조 확인 중 · 기준: {based_on}"),None,None).await;
        let repo=s.repository.clone();
        let scan=tokio::task::spawn_blocking(move||graph::scan(&repo)).await.map_err(|e|e.to_string())??;
        let existing:Vec<String>=scan.get("nodes").and_then(Value::as_array).into_iter().flatten()
            .filter_map(|n|n.get("path").and_then(Value::as_str).map(str::to_owned)).collect();
        let source_root=scan.pointer("/analysis/sourceRoot").and_then(Value::as_str).unwrap_or("").to_owned();
        let observed=planner::observed_imports(&scan);
        let input=read_json(&dir.join("input-plan.json")).await.map_err(|e|e.1)?;
        let current_plan=input.get("plan").cloned().unwrap_or(Value::Null);

        // 2. The job files: task, answer schema, and where the agent runs.
        let task=planning_prompt(&s.repository.to_string_lossy(),&source_root,&instruction,&current_plan,&existing);
        write_json(&dir.join("schema.json"),&planner::output_schema()).await.map_err(|e|e.1)?;
        write_json(&dir.join("run.json"),&json!({"cwd":s.repository,"codex":env::var("CODEX_BIN").unwrap_or_else(|_|"codex".into())})).await.map_err(|e|e.1)?;
        let paths:HashSet<String>=existing.iter().cloned().collect();
        let root=s.repository.join(&source_root);

        // 3. A Herdr workspace in which the agent runs, so its progress can be watched.
        update_job(&s,&id,"running","Herdr workspace 생성 중".into(),None,None).await;
        let created=cli(&s,&["workspace","create","--cwd",&s.repository.to_string_lossy(),"--label",&label,"--no-focus"],Duration::from_secs(35)).await?;
        let data:Value=serde_json::from_str(&created).map_err(|e|format!("Workspace JSON decode failed: {e}: {created}"))?;
        let pane=data.pointer("/result/root_pane/pane_id").and_then(Value::as_str).ok_or_else(||format!("Herdr root pane id missing: {created}"))?.to_owned();
        // A new pane needs a moment before its shell accepts a command.
        tokio::time::sleep(Duration::from_secs(3)).await;

        // 4. Ask, check, and on rejection ask again with the violations. The answer is accepted
        //    only if it parses into the typed proposal and passes every static check.
        let mut feedback=String::new();
        let mut rejection=String::new();
        for attempt in 1..=MAX_ATTEMPTS {
            fs::write(dir.join("prompt.txt"),format!("{task}{feedback}")).await.map_err(|e|e.to_string())?;
            let exit_file=dir.join("exit.json");
            let _=fs::remove_file(&exit_file).await;
            let _=fs::remove_file(dir.join("result.json")).await;
            run_agent_in_pane(&s,&dir,&pane).await?;
            update_job(&s,&id,"running",format!("Codex가 계획을 만드는 중 · 시도 {attempt}/{MAX_ATTEMPTS} · 기준: {based_on} (Herdr {label})"),Some("codex".into()),Some(pane.clone())).await;
            let started=std::time::Instant::now();
            while !exit_file.is_file() {
                if started.elapsed()>AGENT_TIMEOUT {return Err(format!("Codex did not finish within {} minutes.",AGENT_TIMEOUT.as_secs()/60));}
                tokio::time::sleep(Duration::from_secs(2)).await;
            }
            let exit=read_json(&exit_file).await.map_err(|e|e.1)?;
            if exit.get("code").and_then(Value::as_i64)!=Some(0) {
                return Err(format!("Codex exited with an error: {}. See the Herdr workspace {label}.",exit.get("error").and_then(Value::as_str).unwrap_or("non-zero exit code")));
            }
            let answer=fs::read_to_string(dir.join("result.json")).await.map_err(|e|format!("Codex produced no answer: {e}"))?;
            let _=fs::write(dir.join(format!("result-{attempt}.json")),&answer).await;
            let errors=match serde_json::from_str::<planner::Proposal>(&answer) {
                Ok(proposal) => {
                    let errors=planner::validate(&proposal,&planner::Disk{paths:&paths,file_exists:&|file|root.join(file).is_file()},planner::Author::Agent);
                    if errors.is_empty() {return Ok(planner::to_plan(&proposal,&observed));}
                    errors
                }
                Err(e) => vec![format!("The answer does not have the required structure: {e}")],
            };
            let _=write_json(&dir.join(format!("rejected-{attempt}.json")),&json!({"errors":errors})).await;
            rejection=format!("제안이 검증을 통과하지 못해 거절했습니다 ({}건, {attempt}회 시도):\n- {}",errors.len(),errors.join("\n- "));
            // The next attempt gets the rejected answer and exactly what was wrong with it.
            feedback=format!("\n\nYour previous answer was rejected by the checking program. Correct every violation listed here and answer again in full; keep what was not wrong.\nViolations:\n- {}\n\nYour previous answer:\n{answer}\n",errors.join("\n- "));
        }
        Err(rejection)
    }.await;
    match result {
        Ok(plan) => match write_json(&dir.join("proposed-plan.json"), &plan).await {
            Ok(()) => {
                update_job(
                    &s,
                    &id,
                    "completed",
                    "계획 제안이 검증을 통과했습니다. 확인 후 적용하세요.".to_owned(),
                    None,
                    None,
                )
                .await
            }
            Err(e) => update_job(&s, &id, "failed", e.1, None, None).await,
        },
        Err(error) => update_job(&s, &id, "failed", error, None, None).await,
    }
}
