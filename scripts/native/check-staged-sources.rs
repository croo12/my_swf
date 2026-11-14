//! Dependency-free pre-commit check. Read the index, including partial staging and renames.
#[path = "../../apps/native/build_support/source_limits.rs"]
pub mod source_limits;

use std::{path::Path, process::Command};

fn git(arguments: &[&str]) -> Result<Vec<u8>, String> {
    let output = Command::new("git")
        .args(arguments)
        .output()
        .map_err(|error| format!("Cannot run Git: {error}"))?;
    if !output.status.success() {
        return Err(String::from_utf8_lossy(&output.stderr).into_owned());
    }
    Ok(output.stdout)
}

fn check_index() -> Result<(), String> {
    let files = git(&[
        "ls-files",
        "-z",
        "--",
        ":(glob)apps/native/**/*.rs",
        ":(exclude,glob)apps/native/target/**",
    ])?;
    let files = String::from_utf8(files).map_err(|error| error.to_string())?;
    let mut violations = Vec::new();
    for file in files.split('\0').filter(|file| !file.is_empty()) {
        let source = git(&["show", &format!(":{file}")])?;
        let source = String::from_utf8(source)
            .map_err(|error| format!("Cannot read staged Rust source {file}: {error}"))?;
        if let Some(message) = source_limits::violation(Path::new(file), &source) {
            violations.push(message);
        }
    }
    if violations.is_empty() {
        Ok(())
    } else {
        Err(format!(
            "Native Rust files must not exceed {} lines. Split the staged source into modules or shared crates.\n{}",
            source_limits::MAX_RUST_LINES,
            violations.join("\n")
        ))
    }
}

fn main() {
    if let Err(error) = check_index() {
        eprintln!("Commit blocked: {error}");
        std::process::exit(1);
    }
}
