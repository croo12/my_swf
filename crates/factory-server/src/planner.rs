//! The structured result of a planning run, and the static checks it must pass.
//!
//! The AI is not trusted to follow instructions about shape. Its answer is constrained twice:
//! 1. `output_schema()` is handed to the agent as a JSON Schema, so it can only answer in this shape.
//! 2. `validate()` checks the parsed answer against the rules below and against what exists on
//!    disk. A proposal with any violation is rejected as a whole; nothing is repaired or guessed.
use serde::Deserialize;
use serde_json::{json, Value};
use std::collections::{HashMap, HashSet};

pub const MAX_MODULES: usize = 60;
/// File types that can live in the FSD layers. A plan cannot place other code (for example Rust) there.
/// The segment of the shared layer through which every request to the server passes.
const API_SEGMENT: &str = "api";
const SOURCE_EXTENSIONS: &[&str] = &["ts", "tsx", "js", "jsx", "mts", "mjs", "css", "scss", "json"];

/// What the validator needs to know about the repository.
pub struct Disk<'a> {
    /// Module placements found by the scanner (layer/slice/segment).
    pub paths: &'a HashSet<String>,
    /// Whether a file exists, given its path relative to the FSD source root.
    pub file_exists: &'a dyn Fn(&str) -> bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Layer { App, Pages, Widgets, Features, Entities, Shared }

impl Layer {
    pub fn name(self) -> &'static str {
        match self {
            Layer::App => "app", Layer::Pages => "pages", Layer::Widgets => "widgets",
            Layer::Features => "features", Layer::Entities => "entities", Layer::Shared => "shared",
        }
    }
    /// Position from the top: a module may import only from layers with a higher rank.
    fn rank(self) -> usize { self as usize }
    /// app and shared hold segments directly; the other layers group them into slices.
    fn has_slices(self) -> bool { !matches!(self, Layer::App | Layer::Shared) }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Kind {
    /// A module the plan creates.
    New,
    /// A module that exists and must change.
    Modify,
    /// A module that exists, is not changed, and that planned work depends on.
    Existing,
}

impl Kind {
    fn name(self) -> &'static str { match self { Kind::New => "new", Kind::Modify => "modify", Kind::Existing => "existing" } }
}

/// What kind of thing an interface is. `Server` is a request the module sends to the server: the
/// module's interface with the outside, as opposed to what it offers to other modules.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum InterfaceKind { Component, Hook, Function, Store, Constant, Type, Server }

impl InterfaceKind {
    fn name(self) -> &'static str {
        match self {
            InterfaceKind::Component => "component", InterfaceKind::Hook => "hook", InterfaceKind::Function => "function",
            InterfaceKind::Store => "store", InterfaceKind::Constant => "constant", InterfaceKind::Type => "type", InterfaceKind::Server => "server",
        }
    }
    /// Something other modules can use at run time. A type only describes a shape, and a server
    /// request faces the server, so neither makes a module on its own.
    fn is_offered_to_modules(self) -> bool { !matches!(self, InterfaceKind::Type | InterfaceKind::Server) }
}

/// One point where a module meets the outside: something it offers to other modules, or a
/// request it sends to the server.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Interface {
    /// Exported name; for a server request, "METHOD /path" (for example "POST /api/jobs").
    pub name: String,
    pub kind: InterfaceKind,
    /// What it takes: props, arguments, request data. May be empty.
    pub inputs: Vec<String>,
    /// What it gives back: return value, rendered UI, response. May be empty.
    pub outputs: Vec<String>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Module {
    pub id: String,
    pub name: String,
    pub kind: Kind,
    pub layer: Layer,
    /// Slice path below the layer ("home/assignment"); null in app and shared.
    pub slice: Option<String>,
    pub segment: String,
    pub responsibility: String,
    /// What the module does, one behavior per entry.
    pub behaviors: Vec<String>,
    /// Where the module meets the outside: what it offers to other modules and the requests it
    /// sends to the server. Each entry has its own inputs and outputs.
    pub interfaces: Vec<Interface>,
    pub files: Vec<String>,
}

impl Module {
    /// Placement as an FSD path: layer/slice/segment, or layer/segment in app and shared.
    pub fn path(&self) -> String {
        match &self.slice {
            Some(slice) if self.layer.has_slices() => format!("{}/{}/{}", self.layer.name(), slice, self.segment),
            _ => format!("{}/{}", self.layer.name(), self.segment),
        }
    }
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Dependency {
    /// The module that depends.
    pub from: String,
    /// The module it depends on.
    pub to: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Proposal {
    pub summary: String,
    pub modules: Vec<Module>,
    pub dependencies: Vec<Dependency>,
}

/// JSON Schema given to the agent. Every property is required and no extra properties are
/// allowed, so the answer cannot carry anything outside this shape.
pub fn output_schema() -> Value {
    json!({
        "type":"object","additionalProperties":false,
        "required":["summary","modules","dependencies"],
        "properties":{
            "summary":{"type":"string","description":"What the plan builds, in two or three sentences."},
            "modules":{"type":"array","items":{
                "type":"object","additionalProperties":false,
                "required":["id","name","kind","layer","slice","segment","responsibility","behaviors","interfaces","files"],
                "properties":{
                    "id":{"type":"string","description":"Unique id within this plan."},
                    "name":{"type":"string","description":"Short module name."},
                    "kind":{"type":"string","enum":["new","modify","existing"],
                        "description":"new: to be created. modify: exists and must change. existing: exists, unchanged, and a new or modified module depends on it."},
                    "layer":{"type":"string","enum":["app","pages","widgets","features","entities","shared"]},
                    "slice":{"type":["string","null"],"description":"Slice path below the layer, e.g. home/assignment. null in app and shared."},
                    "segment":{"type":"string","description":"Segment inside the slice (or inside app/shared), e.g. ui, model, api."},
                    "responsibility":{"type":"string","description":"The single responsibility of the module."},
                    "behaviors":{"type":"array","items":{"type":"string"},"description":"What the module does, one behavior per entry."},
                    "interfaces":{"type":"array","description":"Where the module meets the outside. One entry per thing it offers to other modules, and one per request it sends to the server.",
                        "items":{"type":"object","additionalProperties":false,"required":["name","kind","inputs","outputs"],
                            "properties":{
                                "name":{"type":"string","description":"Exported name. For kind server: the request as METHOD /path, e.g. POST /api/jobs."},
                                "kind":{"type":"string","enum":["component","hook","function","store","constant","type","server"],
                                    "description":"component, hook, function, store, constant, type: offered to other modules. server: a request this module sends to the server."},
                                "inputs":{"type":"array","items":{"type":"string"},"description":"Props, arguments or request data. Empty when there are none."},
                                "outputs":{"type":"array","items":{"type":"string"},"description":"Return value, rendered UI or response. Empty when there is none."}
                            }}},
                    "files":{"type":"array","items":{"type":"string"},"description":"File names inside the placement."}
                }
            }},
            "dependencies":{"type":"array","items":{
                "type":"object","additionalProperties":false,"required":["from","to"],
                "properties":{
                    "from":{"type":"string","description":"id of the module that imports."},
                    "to":{"type":"string","description":"id of the module it imports from."}
                }
            }}
        }
    })
}

/// A server interface is named by its request: an HTTP method, a space, and a path starting with /.
fn is_server_request(name: &str) -> bool {
    let Some((method, path)) = name.split_once(' ') else { return false; };
    ["GET", "POST", "PUT", "PATCH", "DELETE"].contains(&method) && path.starts_with('/') && !path.contains(char::is_whitespace)
}

fn is_clean_name(part: &str) -> bool {
    !part.is_empty() && part != "." && part != ".." && part.chars().all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.' | '@'))
}

/// Checks a proposal against the rules and against the repository.
/// Returns every violation found, so a rejected proposal can be understood in one read.
pub fn validate(proposal: &Proposal, disk: &Disk) -> Vec<String> {
    let mut errors = vec![];
    let modules = &proposal.modules;
    if modules.is_empty() { errors.push("The plan has no modules.".to_owned()); }
    if modules.len() > MAX_MODULES { errors.push(format!("The plan has {} modules; at most {MAX_MODULES} are allowed.", modules.len())); }
    if !modules.iter().any(|m| m.kind != Kind::Existing) { errors.push("The plan creates or changes nothing: every module is marked existing.".to_owned()); }

    let mut by_id: HashMap<&str, &Module> = HashMap::new();
    for m in modules {
        let label = if m.name.trim().is_empty() { m.id.as_str() } else { m.name.as_str() };
        if m.id.trim().is_empty() { errors.push(format!("Module \"{label}\" has no id.")); }
        else if by_id.insert(m.id.as_str(), m).is_some() { errors.push(format!("Module id \"{}\" is used more than once.", m.id)); }
        if m.name.trim().is_empty() { errors.push(format!("Module \"{}\" has no name.", m.id)); }
        if m.responsibility.trim().is_empty() { errors.push(format!("Module \"{label}\" has no responsibility.")); }

        // Placement: layer is an enum, so code outside the FSD layers cannot be expressed at all.
        if !is_clean_name(&m.segment) { errors.push(format!("Module \"{label}\": segment \"{}\" is not a valid folder name.", m.segment)); }
        match (&m.slice, m.layer.has_slices()) {
            (None, true) => errors.push(format!("Module \"{label}\": layer {} requires a slice.", m.layer.name())),
            (Some(_), false) => errors.push(format!("Module \"{label}\": layer {} has no slices; slice must be null.", m.layer.name())),
            (Some(slice), true) if !slice.split('/').all(is_clean_name) => errors.push(format!("Module \"{label}\": slice \"{slice}\" is not a valid folder path.")),
            _ => {}
        }
        // Files tie the module to the disk: a changed or referenced module must name files that are
        // really at its placement, and a new module must name files that are not there yet.
        if m.files.is_empty() { errors.push(format!("Module \"{label}\" names no files.")); }
        for file in &m.files {
            if !file.split('/').all(is_clean_name) { errors.push(format!("Module \"{label}\": file \"{file}\" is not a valid relative file path.")); continue; }
            let extension = file.rsplit_once('.').map(|(_, ext)| ext).unwrap_or("");
            if !SOURCE_EXTENSIONS.contains(&extension) { errors.push(format!("Module \"{label}\": file \"{file}\" is not a source file that belongs in the FSD layers.")); continue; }
            let inside = format!("{}/{file}", m.path());
            // A segment can be a single file (types.ts); that file sits next to the segment name.
            let stem = file.rsplit_once('.').map(|(stem, _)| stem).unwrap_or(file);
            let as_segment_file = (stem == m.segment).then(|| format!("{}/{file}", m.path().rsplit_once('/').map(|(parent, _)| parent).unwrap_or("")));
            let exists = (disk.file_exists)(&inside) || as_segment_file.as_deref().is_some_and(|path| (disk.file_exists)(path));
            match m.kind {
                Kind::New if exists => errors.push(format!("Module \"{label}\" is marked new but its file {inside} already exists.")),
                Kind::Modify | Kind::Existing if !exists => errors.push(format!("Module \"{label}\" is marked {} but its file {inside} does not exist in the repository.", m.kind.name())),
                _ => {}
            }
        }

        // Behavior and interface are what make a module reviewable; planned work must state them.
        if m.kind != Kind::Existing {
            if m.behaviors.iter().all(|b| b.trim().is_empty()) { errors.push(format!("Module \"{label}\" ({}) states no behavior.", m.kind.name())); }
            if m.interfaces.is_empty() { errors.push(format!("Module \"{label}\" ({}) exposes no interface.", m.kind.name())); }
        }
        if m.behaviors.iter().any(|b| b.trim().is_empty()) { errors.push(format!("Module \"{label}\" has an empty behavior entry.")); }
        // An interface needs a name and a kind; its inputs and outputs are optional.
        let mut interface_names = HashSet::new();
        for interface in &m.interfaces {
            if interface.name.trim().is_empty() { errors.push(format!("Module \"{label}\" has an interface without a name.")); }
            else if !interface_names.insert(interface.name.as_str()) { errors.push(format!("Module \"{label}\" lists the interface \"{}\" more than once.", interface.name)); }
            if interface.inputs.iter().chain(&interface.outputs).any(|s| s.trim().is_empty()) {
                errors.push(format!("Module \"{label}\": interface \"{}\" has an empty input or output entry.", interface.name));
            }
            if interface.kind == InterfaceKind::Server && !is_server_request(&interface.name) {
                errors.push(format!("Module \"{label}\": server interface \"{}\" must be named METHOD /path, for example POST /api/jobs.", interface.name));
            }
        }
        // A file that only declares types is not a module: it does nothing in the module graph.
        // Its types belong in the interfaces of the module that uses them.
        if !m.interfaces.is_empty() && !m.interfaces.iter().any(|i| i.kind.is_offered_to_modules()) {
            errors.push(format!("\"{label}\" offers only types or server requests to other modules, so it is not a module. Put its types and file into the module that uses them."));
        }

        // Claims about the repository are checked against the repository.
        let on_disk = disk.paths.contains(&m.path());
        if m.kind != Kind::New && !on_disk { errors.push(format!("Module \"{label}\" is marked {} but {} does not exist in the repository.", m.kind.name(), m.path())); }
    }

    let mut seen = HashSet::new();
    let mut depended_on: HashSet<&str> = HashSet::new();
    let mut graph: HashMap<&str, Vec<&str>> = HashMap::new();
    for d in &proposal.dependencies {
        let (Some(from), Some(to)) = (by_id.get(d.from.as_str()), by_id.get(d.to.as_str())) else {
            errors.push(format!("Dependency {} -> {} refers to a module that is not in the plan.", d.from, d.to));
            continue;
        };
        if d.from == d.to { errors.push(format!("Module \"{}\" depends on itself.", from.name)); continue; }
        if !seen.insert((d.from.as_str(), d.to.as_str())) { errors.push(format!("Dependency \"{}\" -> \"{}\" is listed more than once.", from.name, to.name)); continue; }
        if from.kind == Kind::Existing { errors.push(format!("Existing module \"{}\" is unchanged, so the plan must not give it dependencies.", from.name)); }
        // FSD import rule: only downward, or within one slice (app and shared count as one slice each).
        let same_slice = from.layer == to.layer && from.slice == to.slice;
        if !(from.layer.rank() < to.layer.rank() || same_slice) {
            errors.push(format!("\"{}\" ({}) may not import \"{}\" ({}): imports go to a lower layer or stay inside one slice.", from.name, from.path(), to.name, to.path()));
        }
        depended_on.insert(d.to.as_str());
        graph.entry(d.from.as_str()).or_default().push(d.to.as_str());
    }
    for m in modules.iter().filter(|m| m.kind == Kind::Existing) {
        if !depended_on.contains(m.id.as_str()) { errors.push(format!("Existing module \"{}\" is in the plan but nothing depends on it.", m.name)); }
    }
    // Requests to the server go through the shared API layer: a module that calls the server is
    // either placed in shared/api or reaches a shared/api module through its dependencies.
    let is_api = |id: &str| by_id.get(id).is_some_and(|m| m.layer == Layer::Shared && m.segment == API_SEGMENT);
    for m in modules.iter().filter(|m| m.interfaces.iter().any(|i| i.kind == InterfaceKind::Server) && !is_api(m.id.as_str())) {
        if !reaches(m.id.as_str(), &graph, &is_api) {
            errors.push(format!("Module \"{}\" sends requests to the server but does not depend, directly or indirectly, on a module in shared/{API_SEGMENT}.", m.name));
        }
    }
    if let Some(id) = find_cycle(&graph) {
        let name = by_id.get(id).map(|m| m.name.as_str()).unwrap_or(id);
        errors.push(format!("The dependencies form a cycle through \"{name}\"."));
    }
    errors
}

/// Whether a module matching `is_target` can be reached from `start` by following dependencies.
fn reaches<'a>(start: &'a str, graph: &HashMap<&'a str, Vec<&'a str>>, is_target: &dyn Fn(&str) -> bool) -> bool {
    let (mut seen, mut stack) = (HashSet::new(), vec![start]);
    while let Some(node) = stack.pop() {
        if !seen.insert(node) { continue; }
        for &next in graph.get(node).into_iter().flatten() {
            if is_target(next) { return true; }
            stack.push(next);
        }
    }
    false
}

/// Returns a module id that lies on a dependency cycle, if there is one.
fn find_cycle<'a>(graph: &HashMap<&'a str, Vec<&'a str>>) -> Option<&'a str> {
    fn visit<'a>(node: &'a str, graph: &HashMap<&'a str, Vec<&'a str>>, done: &mut HashSet<&'a str>, path: &mut HashSet<&'a str>) -> Option<&'a str> {
        if done.contains(node) { return None; }
        if !path.insert(node) { return Some(node); }
        for &next in graph.get(node).into_iter().flatten() {
            if let Some(found) = visit(next, graph, done, path) { return Some(found); }
        }
        path.remove(node);
        done.insert(node);
        None
    }
    let (mut done, mut path) = (HashSet::new(), HashSet::new());
    graph.keys().find_map(|&node| visit(node, graph, &mut done, &mut path))
}

/// Converts a validated proposal into the plan document the client edits.
pub fn to_plan(proposal: &Proposal) -> Value {
    let nodes: Vec<Value> = proposal.modules.iter().map(|m| json!({
        "id":m.id,"name":m.name,"kind":m.kind.name(),"path":m.path(),
        "description":m.responsibility,"behaviors":m.behaviors,
        "interfaces":m.interfaces.iter().map(|i| json!({"name":i.name,"kind":i.kind.name(),"inputs":i.inputs,"outputs":i.outputs})).collect::<Vec<_>>(),
        "files":m.files
    })).collect();
    let edges: Vec<Value> = proposal.dependencies.iter().enumerate().map(|(i, d)| json!({
        "id":format!("dep-{}",i + 1),"from":d.from,"to":d.to,"type":"imports"
    })).collect();
    json!({"schemaVersion":"0.3","version":1,"summary":proposal.summary,"nodes":nodes,"edges":edges,"requests":[]})
}

#[cfg(test)]
mod tests {
    use super::*;

    fn module(id: &str, kind: &str, layer: &str, slice: Option<&str>, segment: &str) -> Value {
        json!({"id":id,"name":id,"kind":kind,"layer":layer,"slice":slice,"segment":segment,"responsibility":"does one thing",
            "behaviors":["does it"],"interfaces":[{"name":"thing","kind":"function","inputs":[],"outputs":["out"]}],"files":["a.ts"]})
    }
    fn proposal(modules: Vec<Value>, dependencies: Vec<(&str, &str)>) -> Proposal {
        let deps: Vec<Value> = dependencies.into_iter().map(|(from, to)| json!({"from":from,"to":to})).collect();
        serde_json::from_value(json!({"summary":"s","modules":modules,"dependencies":deps})).expect("fixture parses")
    }
    /// Validates against a fake repository: `paths` are placements, and every placement holds a.ts.
    fn check(p: &Proposal, paths: &[&str]) -> Vec<String> {
        let paths: HashSet<String> = paths.iter().map(|p| (*p).to_owned()).collect();
        let files: HashSet<String> = paths.iter().map(|p| format!("{p}/a.ts")).collect();
        validate(p, &Disk { paths: &paths, file_exists: &|file| files.contains(file) })
    }
    fn has(errors: &[String], part: &str) -> bool { errors.iter().any(|e| e.contains(part)) }

    #[test]
    fn accepts_a_well_formed_plan() {
        let p = proposal(vec![
            module("ui", "new", "features", Some("cancel-job"), "ui"),
            module("model", "modify", "features", Some("plan-job"), "model"),
            module("api", "existing", "shared", None, "api"),
        ], vec![("ui", "api"), ("model", "api")]);
        assert_eq!(check(&p, &["features/plan-job/model", "shared/api"]), Vec::<String>::new());
    }

    #[test]
    fn the_schema_cannot_express_code_outside_the_fsd_layers() {
        let outside = json!({"summary":"s","dependencies":[],"modules":[module("srv", "new", "server", None, "src")]});
        assert!(serde_json::from_value::<Proposal>(outside).is_err());
    }

    #[test]
    fn rejects_unknown_fields() {
        let mut m = module("a", "new", "features", Some("x"), "ui");
        m["extra"] = json!(1);
        assert!(serde_json::from_value::<Proposal>(json!({"summary":"s","dependencies":[],"modules":[m]})).is_err());
    }

    #[test]
    fn rejects_claims_about_modules_that_are_not_on_disk() {
        let p = proposal(vec![module("m", "modify", "features", Some("ghost"), "model")], vec![]);
        assert!(has(&check(&p, &[]), "does not exist in the repository"));
    }

    #[test]
    fn rejects_slice_rules_violations() {
        let p = proposal(vec![
            module("a", "new", "features", None, "ui"),
            module("b", "new", "shared", Some("x"), "ui"),
            module("c", "new", "pages", Some("../etc"), "ui"),
        ], vec![]);
        let errors = check(&p, &[]);
        assert!(has(&errors, "requires a slice") && has(&errors, "slice must be null") && has(&errors, "not a valid folder path"));
    }

    #[test]
    fn rejects_imports_that_go_upward_or_across_slices() {
        let p = proposal(vec![
            module("entity", "new", "entities", Some("job"), "model"),
            module("feature", "new", "features", Some("cancel"), "ui"),
            module("other", "new", "features", Some("other"), "ui"),
        ], vec![("entity", "feature"), ("feature", "other")]);
        let errors = check(&p, &[]);
        assert_eq!(errors.iter().filter(|e| e.contains("may not import")).count(), 2);
    }

    #[test]
    fn rejects_missing_behavior_interface_and_dangling_dependencies() {
        let mut m = module("a", "new", "features", Some("x"), "ui");
        m["behaviors"] = json!([]);
        m["interfaces"] = json!([]);
        let p = proposal(vec![m], vec![("a", "nowhere")]);
        let errors = check(&p, &[]);
        assert!(has(&errors, "states no behavior") && has(&errors, "exposes no interface") && has(&errors, "not in the plan"));
    }

    #[test]
    fn rejects_files_that_contradict_the_disk_or_do_not_belong_in_fsd() {
        let mut no_files = module("a", "modify", "features", Some("x"), "ui");
        no_files["files"] = json!([]);
        let mut wrong_file = module("b", "modify", "features", Some("x"), "ui");
        wrong_file["files"] = json!(["missing.ts"]);
        let mut rust = module("c", "new", "app", None, "server");
        rust["files"] = json!(["main.rs"]);
        let already_there = module("d", "new", "features", Some("x"), "ui");
        let errors = check(&proposal(vec![no_files, wrong_file, rust, already_there], vec![]), &["features/x/ui"]);
        assert!(has(&errors, "names no files"), "{errors:?}");
        assert!(has(&errors, "missing.ts does not exist"), "{errors:?}");
        assert!(has(&errors, "not a source file that belongs in the FSD layers"), "{errors:?}");
        assert!(has(&errors, "already exists"), "{errors:?}");
    }

    #[test]
    fn an_interface_needs_a_name_but_not_inputs_or_outputs() {
        let mut bare = module("a", "new", "features", Some("x"), "ui");
        bare["interfaces"] = json!([{"name":"Thing","kind":"component","inputs":[],"outputs":[]}]);
        assert_eq!(check(&proposal(vec![bare], vec![]), &[]), Vec::<String>::new());
        let mut bad = module("b", "new", "features", Some("x"), "ui");
        bad["interfaces"] = json!([{"name":"","kind":"hook","inputs":[],"outputs":[]},{"name":"A","kind":"hook","inputs":[],"outputs":[]},{"name":"A","kind":"hook","inputs":[],"outputs":[]}]);
        let errors = check(&proposal(vec![bad], vec![]), &[]);
        assert!(has(&errors, "without a name") && has(&errors, "more than once"), "{errors:?}");
        let unknown_kind = json!({"summary":"s","dependencies":[],"modules":[{"id":"a","name":"a","kind":"new","layer":"shared","slice":null,"segment":"lib","responsibility":"r","behaviors":["b"],"interfaces":[{"name":"x","kind":"service","inputs":[],"outputs":[]}],"files":["a.ts"]}]});
        assert!(serde_json::from_value::<Proposal>(unknown_kind).is_err());
    }

    #[test]
    fn a_file_of_types_is_not_a_module() {
        let mut types = module("contract", "new", "features", Some("x"), "model");
        types["interfaces"] = json!([{"name":"RunContract","kind":"type","inputs":[],"outputs":[]},{"name":"RunResult","kind":"type","inputs":[],"outputs":[]}]);
        assert!(has(&check(&proposal(vec![types], vec![]), &[]), "so it is not a module"));
        // Types next to something usable are fine: they are part of that module's interface.
        let mut with_types = module("model", "new", "features", Some("x"), "model");
        with_types["interfaces"] = json!([{"name":"RunContract","kind":"type","inputs":[],"outputs":[]},{"name":"runSentence","kind":"function","inputs":["sentence: string"],"outputs":["RunResult"]}]);
        assert_eq!(check(&proposal(vec![with_types], vec![]), &[]), Vec::<String>::new());
    }

    #[test]
    fn server_requests_are_interfaces_and_pass_through_shared_api() {
        let request = json!({"name":"POST /api/jobs/{id}/cancel","kind":"server","inputs":["id"],"outputs":["job"]});
        let function = json!({"name":"cancelJob","kind":"function","inputs":[],"outputs":[]});
        let mut model = module("model", "new", "features", Some("cancel"), "model");
        model["interfaces"] = json!([function, request]);
        let api = module("api", "existing", "shared", None, "api");
        let lib = module("lib", "existing", "shared", None, "lib");
        // Directly through shared/api, or indirectly through another module: accepted.
        assert_eq!(check(&proposal(vec![model.clone(), api.clone()], vec![("model", "api")]), &["shared/api"]), Vec::<String>::new());
        let mut ui = module("ui", "new", "features", Some("cancel"), "ui");
        ui["interfaces"] = json!([{"name":"CancelButton","kind":"component","inputs":[],"outputs":[]}, {"name":"GET /api/jobs/{id}","kind":"server","inputs":[],"outputs":[]}]);
        let plain_model = module("model", "new", "features", Some("cancel"), "model");
        assert_eq!(check(&proposal(vec![ui, plain_model, api.clone()], vec![("ui", "model"), ("model", "api")]), &["shared/api"]), Vec::<String>::new());
        // Bypassing shared/api: rejected.
        let errors = check(&proposal(vec![model.clone(), lib], vec![("model", "lib")]), &["shared/lib"]);
        assert!(has(&errors, "does not depend, directly or indirectly, on a module in shared/api"), "{errors:?}");
        // A server interface that is not named as a request: rejected.
        let mut misnamed = model;
        misnamed["interfaces"] = json!([{"name":"cancelJob","kind":"function","inputs":[],"outputs":[]},{"name":"cancel the job","kind":"server","inputs":[],"outputs":[]}]);
        assert!(has(&check(&proposal(vec![misnamed, api], vec![("model", "api")]), &["shared/api"]), "must be named METHOD /path"));
    }

    #[test]
    fn rejects_unused_existing_modules_and_cycles() {
        let p = proposal(vec![
            module("a", "new", "features", Some("x"), "ui"),
            module("b", "new", "features", Some("x"), "model"),
            module("lib", "existing", "shared", None, "lib"),
        ], vec![("a", "b"), ("b", "a")]);
        let errors = check(&p, &["shared/lib"]);
        assert!(has(&errors, "nothing depends on it") && has(&errors, "form a cycle"));
    }
}
