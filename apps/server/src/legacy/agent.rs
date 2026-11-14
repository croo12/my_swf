use crate::state::AppState;
use serde_json::{json, Value};
use std::{env, path::Path, time::Duration};
use tokio::process::Command;

pub(crate) async fn cli(s: &AppState, args: &[&str], max_wait: Duration) -> Result<String, String> {
    let child = Command::new(&s.herdr_bin)
        .args(args)
        .current_dir(&s.repository)
        .kill_on_drop(true)
        .output();
    let output = tokio::time::timeout(max_wait, child)
        .await
        .map_err(|_| format!("Herdr timeout: {}", args.join(" ")))?
        .map_err(|e| format!("Cannot run Herdr: {e}"))?;
    if !output.status.success() {
        return Err(format!(
            "Herdr exited {:?}: {}",
            output.status.code(),
            String::from_utf8_lossy(&output.stderr)
                .chars()
                .take(1500)
                .collect::<String>()
        ));
    }
    Ok(String::from_utf8_lossy(&output.stdout).to_string())
}
/// Runs the agent in the Herdr pane so its progress can be watched, by starting this same binary
/// as `factory-server run-codex <job-dir>`. Going through our own binary keeps the command line
/// free of quoting, which differs between the shells a pane may run.
pub(crate) async fn run_agent_in_pane(s: &AppState, dir: &Path, pane: &str) -> Result<(), String> {
    let exe = env::current_exe().map_err(|e| format!("Cannot locate the server binary: {e}"))?;
    let (exe, job_dir) = (
        exe.to_string_lossy().to_string(),
        dir.to_string_lossy().to_string(),
    );
    if exe.contains(char::is_whitespace) || job_dir.contains(char::is_whitespace) {
        return Err(format!(
            "Paths with spaces cannot be passed to the Herdr pane: {exe} / {job_dir}"
        ));
    }
    cli(
        s,
        &["pane", "run", pane, &exe, "run-codex", &job_dir],
        Duration::from_secs(35),
    )
    .await?;
    Ok(())
}

/// Entry point of `factory-server run-codex <job-dir>`. Runs `codex exec` with the job's prompt on
/// stdin, constrained to the job's output schema and to a read-only sandbox, and records the exit
/// code in exit.json for the server to pick up.
pub(crate) fn run_codex(job_dir: &Path) -> i32 {
    let run = || -> Result<i32, String> {
        let settings: Value = serde_json::from_slice(
            &std::fs::read(job_dir.join("run.json")).map_err(|e| format!("run.json: {e}"))?,
        )
        .map_err(|e| format!("run.json: {e}"))?;
        let cwd = settings
            .get("cwd")
            .and_then(Value::as_str)
            .ok_or("run.json has no cwd")?;
        // The pane's shell does not have the server's environment, so the server passes its
        // CODEX_BIN setting through run.json.
        let codex = settings
            .get("codex")
            .and_then(Value::as_str)
            .unwrap_or("codex")
            .to_owned();
        let prompt = std::fs::File::open(job_dir.join("prompt.txt"))
            .map_err(|e| format!("prompt.txt: {e}"))?;
        // On Windows codex is a .cmd shim, which only the command interpreter can start.
        let mut command = if cfg!(windows) {
            let mut c = std::process::Command::new("cmd");
            c.args(["/C", &codex]);
            c
        } else {
            std::process::Command::new(&codex)
        };
        let status = command
            .arg("exec")
            .args([
                "--sandbox",
                "read-only",
                "--ephemeral",
                "--skip-git-repo-check",
                "-c",
                "check_for_update_on_startup=false",
                "-C",
                cwd,
            ])
            .arg("--output-schema")
            .arg(job_dir.join("schema.json"))
            .arg("--output-last-message")
            .arg(job_dir.join("result.json"))
            .arg("-")
            .stdin(prompt)
            .status()
            .map_err(|e| format!("Cannot start {codex}: {e}"))?;
        Ok(status.code().unwrap_or(-1))
    };
    let (code, error) = match run() {
        Ok(code) => (code, None),
        Err(e) => {
            eprintln!("{e}");
            (-1, Some(e))
        }
    };
    let _ = std::fs::write(
        job_dir.join("exit.json"),
        json!({"code":code,"error":error}).to_string(),
    );
    code
}
