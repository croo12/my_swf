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

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Interface {
    pub inputs: Vec<String>,
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
    pub interface: Interface,
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
    let strings = json!({"type":"array","items":{"type":"string"}});
    json!({
        "type":"object","additionalProperties":false,
        "required":["summary","modules","dependencies"],
        "properties":{
            "summary":{"type":"string","description":"What the plan builds, in two or three sentences."},
            "modules":{"type":"array","items":{
                "type":"object","additionalProperties":false,
                "required":["id","name","kind","layer","slice","segment","responsibility","behaviors","interface","files"],
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
                    "interface":{"type":"object","additionalProperties":false,"required":["inputs","outputs"],
                        "properties":{"inputs":strings.clone(),"outputs":strings.clone()},
                        "description":"What the module takes and what it provides: props, arguments, events, exported values."},
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
            if m.interface.inputs.is_empty() && m.interface.outputs.is_empty() { errors.push(format!("Module \"{label}\" ({}) has an empty interface.", m.kind.name())); }
        }
        if m.behaviors.iter().chain(&m.interface.inputs).chain(&m.interface.outputs).any(|s| s.trim().is_empty()) {
            errors.push(format!("Module \"{label}\" has an empty behavior or interface entry."));
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
    if let Some(id) = find_cycle(&graph) {
        let name = by_id.get(id).map(|m| m.name.as_str()).unwrap_or(id);
        errors.push(format!("The dependencies form a cycle through \"{name}\"."));
    }
    errors
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
        "inputs":m.interface.inputs,"outputs":m.interface.outputs,"files":m.files,"x":0,"y":0
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
            "behaviors":["does it"],"interface":{"inputs":["in"],"outputs":["out"]},"files":["a.ts"]})
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
        m["interface"] = json!({"inputs":[],"outputs":[]});
        let p = proposal(vec![m], vec![("a", "nowhere")]);
        let errors = check(&p, &[]);
        assert!(has(&errors, "states no behavior") && has(&errors, "empty interface") && has(&errors, "not in the plan"));
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
