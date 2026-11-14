//! Compatibility CLI commands, separate from server startup.
use crate::legacy::{agent, planner};
use factory_repository::graph;
use serde_json::Value;
use std::{collections::HashSet, env, path::Path};

pub(crate) fn run(args: &[String]) -> Result<bool, Box<dyn std::error::Error>> {
    // `factory-server check-proposal <file>` runs the validator on a proposal by hand, against the
    // repository in FACTORY_REPO_PATH. It prints every violation and exits with 1 if there is any.
    if args.get(1).map(String::as_str) == Some("check-proposal") {
        let file = args
            .get(2)
            .ok_or("Usage: factory-server check-proposal <proposal.json>")?;
        let repo = std::path::absolute(
            env::var("FACTORY_REPO_PATH").map_err(|_| "Set FACTORY_REPO_PATH")?,
        )?;
        let scan = graph::scan(&repo)?;
        let paths: HashSet<String> = scan
            .get("nodes")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .filter_map(|n| n.get("path").and_then(Value::as_str).map(str::to_owned))
            .collect();
        let root = repo.join(
            scan.pointer("/analysis/sourceRoot")
                .and_then(Value::as_str)
                .unwrap_or(""),
        );
        let proposal: planner::Proposal = serde_json::from_str(&std::fs::read_to_string(file)?)?;
        let errors = planner::validate(
            &proposal,
            &planner::Disk {
                paths: &paths,
                file_exists: &|file| root.join(file).is_file(),
            },
            planner::Author::Agent,
        );
        for error in &errors {
            println!("- {error}");
        }
        println!("{} violation(s)", errors.len());
        // With a second argument, also write the plan the client would get.
        if let Some(out) = args.get(3) {
            std::fs::write(
                out,
                serde_json::to_vec_pretty(&planner::to_plan(
                    &proposal,
                    &planner::observed_imports(&scan),
                ))?,
            )?;
        }
        std::process::exit(if errors.is_empty() { 0 } else { 1 });
    }
    // `factory-server slice-modules <layer/slice>` prints the modules of a slice of FACTORY_REPO_PATH.
    if args.get(1).map(String::as_str) == Some("slice-modules") {
        let key = args
            .get(2)
            .ok_or("Usage: factory-server slice-modules <layer/slice>")?;
        let repo = std::path::absolute(
            env::var("FACTORY_REPO_PATH").map_err(|_| "Set FACTORY_REPO_PATH")?,
        )?;
        println!(
            "{}",
            serde_json::to_string_pretty(&graph::slice_modules(&repo, &graph::scan(&repo)?, key)?)?
        );
        return Ok(true);
    }
    if args.get(1).map(String::as_str) == Some("run-codex") {
        let job_dir = args
            .get(2)
            .ok_or("Usage: factory-server run-codex <job-dir>")?;
        std::process::exit(agent::run_codex(Path::new(job_dir)));
    }
    Ok(false)
}
