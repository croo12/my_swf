use super::*;
use serde_json::{json, Value};

/// Turns the plan document the client edits back into a proposal, so a plan drawn by a person is
/// checked by the same rules. Code the server added around the plan is left out.
pub fn proposal_from_plan(plan: &Value) -> Result<Proposal, String> {
    let observed = |value: &Value| {
        value
            .get("observed")
            .and_then(Value::as_bool)
            .unwrap_or(false)
    };
    let text = |value: &Value, key: &str| {
        value
            .get(key)
            .and_then(Value::as_str)
            .unwrap_or("")
            .trim()
            .to_owned()
    };
    let list = |value: &Value, key: &str| -> Vec<Value> {
        value
            .get(key)
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default()
    };
    let nodes: Vec<Value> = list(plan, "nodes")
        .into_iter()
        .filter(|n| !observed(n))
        .collect();
    let ids: HashSet<String> = nodes.iter().map(|n| text(n, "id")).collect();
    let modules: Vec<Value> = nodes.iter().map(|n| {
        let path = text(n, "path");
        let parts: Vec<&str> = path.split('/').filter(|part| !part.is_empty()).collect();
        let (layer, segment) = (parts.first().copied().unwrap_or(""), parts.last().copied().unwrap_or(""));
        let slice = (parts.len() > 2).then(|| parts[1..parts.len() - 1].join("/"));
        let name = text(n, "name");
        let interfaces: Vec<Value> = list(n, "interfaces").iter().map(|i| json!({
            "name":text(i, "name"),"kind":text(i, "kind"),"condition":text(i, "condition"),"result":text(i, "result"),
            "inputs":list(i, "inputs"),"outputs":list(i, "outputs")
        })).collect();
        json!({"id":text(n, "id"),"name":if name.is_empty() { segment.to_owned() } else { name },"kind":text(n, "kind"),"layer":layer,"slice":slice,"segment":segment,
            "responsibility":text(n, "description"),"interfaces":interfaces,"files":list(n, "files")})
    }).collect();
    let dependencies: Vec<Value> = list(plan, "edges")
        .into_iter()
        .filter(|e| !observed(e) && ids.contains(&text(e, "from")) && ids.contains(&text(e, "to")))
        .map(|e| json!({"from":text(&e, "from"),"to":text(&e, "to"),"uses":list(&e, "uses")}))
        .collect();
    serde_json::from_value(
        json!({"summary":text(plan, "summary"),"modules":modules,"dependencies":dependencies}),
    )
    .map_err(|e| format!("The plan cannot be read as modules and dependencies: {e}"))
}

/// Converts a validated proposal into the plan document the client edits, and adds the unplanned
/// code that the planned modules are connected to in the repository.
pub fn to_plan(proposal: &Proposal, observed: &[ObservedImport]) -> Value {
    let (context_nodes, context_edges) = surroundings(proposal, observed);
    let mut nodes: Vec<Value> = proposal.modules.iter().map(|m| json!({
        "id":m.id,"name":m.name,"kind":m.kind.name(),"path":m.path(),"description":m.responsibility,
        "interfaces":m.interfaces.iter().map(|i| json!({"name":i.name,"kind":i.kind.name(),"condition":i.condition,"result":i.result,"inputs":i.inputs,"outputs":i.outputs})).collect::<Vec<_>>(),
        "files":m.files
    })).collect();
    let mut edges: Vec<Value> = proposal
        .dependencies
        .iter()
        .enumerate()
        .map(|(i, d)| {
            json!({
                "id":format!("dep-{}",i + 1),"from":d.from,"to":d.to,"type":"imports","uses":d.uses
            })
        })
        .collect();
    nodes.extend(context_nodes);
    edges.extend(context_edges);
    json!({"schemaVersion":"0.3","version":1,"summary":proposal.summary,"nodes":nodes,"edges":edges,"requests":[]})
}
