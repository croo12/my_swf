use serde_json::{json, Value};
use std::{path::Path, time::Duration};
use tokio::process::Command;

/// Brings the checked-out branch up to date with its upstream before the repository is read, so
/// plans and analyses are based on the newest code. Fast-forward only: the agent runs read-only and
/// this checkout is not edited by hand, so there is nothing to merge. A checkout that cannot be
/// fast-forwarded (no network, no upstream, local commits) is left as it is and the reason is
/// returned, so the caller can say what the result is based on.
pub(crate) async fn pull_latest(repo: &Path) -> Result<(), String> {
    // Never wait for a password prompt: there is nobody to answer it.
    let run = Command::new("git")
        .arg("-C")
        .arg(repo)
        .args(["pull", "--ff-only"])
        .env("GIT_TERMINAL_PROMPT", "0")
        .kill_on_drop(true)
        .output();
    let output = tokio::time::timeout(Duration::from_secs(90), run)
        .await
        .map_err(|_| "git pull did not finish within 90 seconds".to_owned())?
        .map_err(|e| format!("cannot run git: {e}"))?;
    if output.status.success() {
        return Ok(());
    }
    let reason = String::from_utf8_lossy(&output.stderr);
    // Git explains the failure on its first "fatal:" or "error:" line; what follows is advice.
    let line = reason
        .lines()
        .map(str::trim)
        .find(|line| line.starts_with("fatal:") || line.starts_with("error:"))
        .or_else(|| reason.lines().map(str::trim).find(|line| !line.is_empty()))
        .unwrap_or("git pull failed");
    Err(line.chars().take(200).collect())
}
/// What the repository's working tree is on right now: the checked-out branch, its commit, and
/// any uncommitted changes.
pub(crate) async fn git_state(repo: &Path) -> Value {
    async fn git(repo: &Path, args: &[&str]) -> Option<String> {
        let run = Command::new("git")
            .arg("-C")
            .arg(repo)
            .args(args)
            .kill_on_drop(true)
            .output();
        let output = tokio::time::timeout(Duration::from_secs(10), run)
            .await
            .ok()?
            .ok()?;
        output
            .status
            .success()
            .then(|| String::from_utf8_lossy(&output.stdout).trim().to_owned())
    }
    let Some(commit) = git(repo, &["rev-parse", "--short", "HEAD"]).await else {
        return Value::Null;
    };
    let branch = git(repo, &["rev-parse", "--abbrev-ref", "HEAD"])
        .await
        .unwrap_or_default();
    let committed_at = git(repo, &["log", "-1", "--format=%cI"])
        .await
        .unwrap_or_default();
    let changed = git(repo, &["status", "--porcelain"])
        .await
        .map(|out| out.lines().count())
        .unwrap_or(0);
    json!({"branch":branch,"commit":commit,"committedAt":committed_at,"uncommittedFiles":changed})
}
