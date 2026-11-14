mod conversion;
mod dependencies;
mod interfaces;
mod modules;
use super::*;
use serde_json::{json, Value};

/// An interface whose signature is its own: the output type is derived from the name.
fn interface(name: &str, kind: &str) -> Value {
    let type_name: String = name.chars().filter(char::is_ascii_alphanumeric).collect();
    json!({"name":name,"kind":kind,"condition":"the caller asks","result":"it answers","inputs":["id: string"],"outputs":[format!("Promise<{type_name}Result>")]})
}
fn module(id: &str, kind: &str, layer: &str, slice: Option<&str>, segment: &str) -> Value {
    json!({"id":id,"name":id,"kind":kind,"layer":layer,"slice":slice,"segment":segment,"responsibility":"does one thing",
        "interfaces":[interface(&format!("run_{id}"), "function")],"files":["a.ts"]})
}
fn proposal(modules: Vec<Value>, dependencies: Vec<(&str, &str)>) -> Proposal {
    // By default a dependency uses every interface its target lists.
    let deps: Vec<Value> = dependencies
        .into_iter()
        .map(|(from, to)| {
            let uses: Vec<Value> = modules
                .iter()
                .filter(|m| m["id"] == to)
                .flat_map(|m| m["interfaces"].as_array().cloned().unwrap_or_default())
                .filter(|i| i["kind"] != "server")
                .map(|i| i["name"].clone())
                .collect();
            json!({"from":from,"to":to,"uses":uses})
        })
        .collect();
    serde_json::from_value(json!({"summary":"s","modules":modules,"dependencies":deps}))
        .expect("fixture parses")
}
fn import(from: &str, file: &str, to: &str, provider: &str, name: &str) -> ObservedImport {
    ObservedImport {
        from: from.to_owned(),
        file: file.to_owned(),
        to: to.to_owned(),
        provider: provider.to_owned(),
        name: name.to_owned(),
    }
}

/// Validates against a fake repository: `paths` are placements, and every placement holds a.ts.
fn check(p: &Proposal, paths: &[&str]) -> Vec<String> {
    let paths: HashSet<String> = paths.iter().map(|p| (*p).to_owned()).collect();
    let files: HashSet<String> = paths.iter().map(|p| format!("{p}/a.ts")).collect();
    validate(
        p,
        &Disk {
            paths: &paths,
            file_exists: &|file| files.contains(file),
        },
        Author::Agent,
    )
}
/// Validates one new module whose only interface is the given one.
fn check_interface(interface: Value) -> Vec<String> {
    let mut m = module("a", "new", "features", Some("x"), "ui");
    m["interfaces"] = json!([interface]);
    check(&proposal(vec![m], vec![]), &[])
}
fn has(errors: &[String], part: &str) -> bool {
    errors.iter().any(|e| e.contains(part))
}
const NONE: Vec<String> = Vec::new();
