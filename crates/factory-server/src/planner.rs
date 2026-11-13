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
/// The segment of the shared layer through which every request to the server passes.
const API_SEGMENT: &str = "api";
/// File types that can live in the FSD layers. A plan cannot place other code (for example Rust) there.
const SOURCE_EXTENSIONS: &[&str] = &["ts", "tsx", "js", "jsx", "mts", "mjs", "css", "scss", "json"];

// Limits on free text and on list sizes, counted in characters. Prose is where a plan becomes
// vague, so there is little room for it: one sentence of responsibility per module and one
// condition/result pair per interface. Everything else is names and types.
const MAX_SUMMARY: usize = 300;
const MAX_NAME: usize = 40;
const MAX_RESPONSIBILITY: usize = 100;
const MAX_CONDITION: usize = 80;
const MAX_RESULT: usize = 100;
const MAX_INTERFACES: usize = 12;
const MAX_PORTS: usize = 8;
const MAX_PORT: usize = 120;
const MAX_FILES: usize = 12;
/// Words that may stand next to another word inside a TypeScript type expression.
const TYPE_KEYWORDS: &[&str] = &["keyof", "typeof", "readonly", "extends", "infer", "in", "is", "new", "unique", "asserts", "abstract"];

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
    /// Something that does work when another module uses it. A type describes a shape, a constant
    /// is a value, and a server request faces the server; none of them makes a module on its own.
    fn acts(self) -> bool { matches!(self, InterfaceKind::Component | InterfaceKind::Hook | InterfaceKind::Function | InterfaceKind::Store) }
}

/// One point where a module meets the outside: something it offers to other modules, or a
/// request it sends to the server.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Interface {
    /// Exported name; for a server request, "METHOD /path" (for example "POST /api/jobs").
    pub name: String,
    pub kind: InterfaceKind,
    /// Behavior as a condition and a result, the two halves of an EARS requirement ("when
    /// <condition>, the module shall <result>"). `condition` is empty for something that always holds.
    pub condition: String,
    pub result: String,
    /// What it takes, each written `name: Type`. May be empty.
    pub inputs: Vec<String>,
    /// What it gives back, each written `name: Type` or as a bare type. May be empty.
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
    /// One sentence.
    pub responsibility: String,
    /// Where the module meets the outside: what it offers to other modules and the requests it
    /// sends to the server. Each entry states its own behavior, inputs and outputs.
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
            "summary":{"type":"string","description":format!("What the plan builds. At most {MAX_SUMMARY} characters.")},
            "modules":{"type":"array","items":{
                "type":"object","additionalProperties":false,
                "required":["id","name","kind","layer","slice","segment","responsibility","interfaces","files"],
                "properties":{
                    "id":{"type":"string","description":"Unique id within this plan."},
                    "name":{"type":"string","description":format!("Module name. At most {MAX_NAME} characters.")},
                    "kind":{"type":"string","enum":["new","modify","existing"],
                        "description":"new: to be created. modify: exists and must change. existing: exists, unchanged, and a new or modified module depends on it."},
                    "layer":{"type":"string","enum":["app","pages","widgets","features","entities","shared"]},
                    "slice":{"type":["string","null"],"description":"Slice path below the layer, e.g. home/assignment. null in app and shared."},
                    "segment":{"type":"string","description":"Segment inside the slice (or inside app/shared), e.g. ui, model, api."},
                    "responsibility":{"type":"string","description":format!("The single responsibility of the module, as one sentence of at most {MAX_RESPONSIBILITY} characters.")},
                    "interfaces":{"type":"array","description":format!("Where the module meets the outside. One entry per thing it offers to other modules, and one per request it sends to the server. At most {MAX_INTERFACES}."),
                        "items":{"type":"object","additionalProperties":false,"required":["name","kind","condition","result","inputs","outputs"],
                            "properties":{
                                "name":{"type":"string","description":"Exported name. For kind server: the request as METHOD /path, e.g. POST /api/jobs."},
                                "kind":{"type":"string","enum":["component","hook","function","store","constant","type","server"],
                                    "description":"component, hook, function, store, constant, type: offered to other modules. server: a request this module sends to the server."},
                                "condition":{"type":"string","description":format!("The condition or trigger of the behavior. Empty when the result always holds. At most {MAX_CONDITION} characters.")},
                                "result":{"type":"string","description":format!("What happens under that condition: one observable result. At most {MAX_RESULT} characters. May be empty for kind type and constant.")},
                                "inputs":{"type":"array","items":{"type":"string"},"description":format!("Props, arguments or request data, each written name: Type with a TypeScript type, e.g. id: string. At most {MAX_PORTS}. Empty when there are none.")},
                                "outputs":{"type":"array","items":{"type":"string"},"description":format!("Return value or response, each a TypeScript type or name: Type, e.g. Promise<boolean>. At most {MAX_PORTS}. Empty when there is none.")}
                            }}},
                    "files":{"type":"array","items":{"type":"string"},"description":format!("File names inside the placement. At most {MAX_FILES}.")}
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

fn is_identifier(text: &str) -> bool {
    let mut chars = text.chars();
    chars.next().is_some_and(|c| c.is_ascii_alphabetic() || c == '_' || c == '$') && chars.all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '$')
}

/// Whether the text can be a TypeScript type expression. This is not a parser: it accepts only
/// ASCII, requires balanced brackets, and rejects two plain words in a row (as in a phrase)
/// unless one of them is a type keyword such as `keyof`. That is enough to keep prose out.
fn is_type_expression(text: &str) -> bool {
    let text = text.trim();
    if text.is_empty() || !text.is_ascii() { return false; }
    let mut open: Vec<char> = vec![];
    let mut previous_word: Option<String> = None;
    let mut previous_char = ' ';
    let mut chars = text.chars().peekable();
    while let Some(c) = chars.next() {
        let before = std::mem::replace(&mut previous_char, c);
        match c {
            // String literal types may contain anything.
            '\'' | '"' | '`' => { for inner in chars.by_ref() { if inner == c { break; } } previous_word = None; }
            '(' | '[' | '{' | '<' => { open.push(c); previous_word = None; }
            // "=>" is the arrow of a function type, not a closing bracket.
            '>' if before == '=' => previous_word = None,
            ')' | ']' | '}' | '>' => {
                let expected = match c { ')' => '(', ']' => '[', '}' => '{', _ => '<' };
                if open.pop() != Some(expected) { return false; }
                previous_word = None;
            }
            c if c.is_ascii_alphanumeric() || c == '_' || c == '$' => {
                let mut word = c.to_string();
                while let Some(&next) = chars.peek() {
                    if next.is_ascii_alphanumeric() || next == '_' || next == '$' { word.push(next); chars.next(); } else { break; }
                }
                // Two words in a row are fine only around a type keyword: "keyof T", "T extends U".
                if previous_word.as_deref().is_some_and(|before| !TYPE_KEYWORDS.contains(&before) && !TYPE_KEYWORDS.contains(&word.as_str())) { return false; }
                previous_word = Some(word);
            }
            c if c.is_whitespace() => {}
            _ => previous_word = None,
        }
    }
    open.is_empty()
}

/// `name: Type`, with an optional `?` after the name.
fn is_typed_name(entry: &str) -> bool {
    entry.split_once(':').is_some_and(|(name, ty)| is_identifier(name.trim().trim_end_matches('?')) && is_type_expression(ty))
}

/// One sentence: no line breaks, and no sentence end followed by more text.
fn is_one_sentence(text: &str) -> bool {
    let text = text.trim().trim_end_matches(['.', '。']);
    !text.contains('\n') && !text.contains(". ") && !text.contains("。")
}

fn length(text: &str) -> usize { text.trim().chars().count() }

/// Inputs or outputs with whitespace removed and in a fixed order, for comparing two signatures.
fn normalized(entries: &[String]) -> Vec<String> {
    let mut entries: Vec<String> = entries.iter().map(|e| e.split_whitespace().collect()).collect();
    entries.sort();
    entries
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
    if length(&proposal.summary) > MAX_SUMMARY { errors.push(format!("The summary has {} characters; at most {MAX_SUMMARY} are allowed.", length(&proposal.summary))); }

    let mut by_id: HashMap<&str, &Module> = HashMap::new();
    for m in modules {
        let label = if m.name.trim().is_empty() { m.id.as_str() } else { m.name.as_str() };
        if m.id.trim().is_empty() { errors.push(format!("Module \"{label}\" has no id.")); }
        else if by_id.insert(m.id.as_str(), m).is_some() { errors.push(format!("Module id \"{}\" is used more than once.", m.id)); }
        if m.name.trim().is_empty() { errors.push(format!("Module \"{}\" has no name.", m.id)); }
        if m.responsibility.trim().is_empty() { errors.push(format!("Module \"{label}\" has no responsibility.")); }
        if length(&m.name) > MAX_NAME { errors.push(format!("Module name \"{label}\" has {} characters; at most {MAX_NAME} are allowed.", length(&m.name))); }
        if length(&m.responsibility) > MAX_RESPONSIBILITY { errors.push(format!("Module \"{label}\": the responsibility has {} characters; at most {MAX_RESPONSIBILITY} are allowed.", length(&m.responsibility))); }
        if !is_one_sentence(&m.responsibility) { errors.push(format!("Module \"{label}\": the responsibility must be one sentence.")); }
        if m.interfaces.len() > MAX_INTERFACES { errors.push(format!("Module \"{label}\" has {} interfaces; at most {MAX_INTERFACES} are allowed. Split the module.", m.interfaces.len())); }
        if m.files.len() > MAX_FILES { errors.push(format!("Module \"{label}\" names {} files; at most {MAX_FILES} are allowed. Split the module.", m.files.len())); }

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

        // Interfaces are what make a module reviewable; planned work must state them.
        if m.kind != Kind::Existing && m.interfaces.is_empty() { errors.push(format!("Module \"{label}\" ({}) exposes no interface.", m.kind.name())); }
        let mut interface_names = HashSet::new();
        for interface in &m.interfaces {
            let at = format!("Module \"{label}\": interface \"{}\"", interface.name);
            if interface.name.trim().is_empty() { errors.push(format!("Module \"{label}\" has an interface without a name.")); }
            else if !interface_names.insert(interface.name.as_str()) { errors.push(format!("Module \"{label}\" lists the interface \"{}\" more than once.", interface.name)); }
            if interface.kind == InterfaceKind::Server && !is_server_request(&interface.name) {
                errors.push(format!("Module \"{label}\": server interface \"{}\" must be named METHOD /path, for example POST /api/jobs.", interface.name));
            }

            // Behavior: one condition and one result. Planned work must state the result of
            // everything that does something; a type or a constant does nothing by itself.
            let acts = !matches!(interface.kind, InterfaceKind::Type | InterfaceKind::Constant);
            if m.kind != Kind::Existing && acts && interface.result.trim().is_empty() { errors.push(format!("{at} states no result.")); }
            if length(&interface.condition) > MAX_CONDITION { errors.push(format!("{at}: the condition has {} characters; at most {MAX_CONDITION} are allowed.", length(&interface.condition))); }
            if length(&interface.result) > MAX_RESULT { errors.push(format!("{at}: the result has {} characters; at most {MAX_RESULT} are allowed.", length(&interface.result))); }
            if !is_one_sentence(&interface.condition) || !is_one_sentence(&interface.result) { errors.push(format!("{at}: the condition and the result must each be one sentence.")); }

            // Inputs and outputs are names and types, not descriptions.
            if interface.inputs.len() > MAX_PORTS || interface.outputs.len() > MAX_PORTS { errors.push(format!("{at} has more than {MAX_PORTS} inputs or outputs.")); }
            for input in &interface.inputs {
                if length(input) > MAX_PORT { errors.push(format!("{at}: input \"{input}\" is longer than {MAX_PORT} characters.")); }
                else if !is_typed_name(input) { errors.push(format!("{at}: input \"{input}\" must be written name: Type with a TypeScript type, for example id: string.")); }
            }
            for output in &interface.outputs {
                let restated = output.trim() == interface.name.trim() || output.trim() == format!("typeof {}", interface.name.trim());
                if restated { errors.push(format!("{at}: output \"{output}\" only repeats the name. State the type itself, or leave the outputs empty.")); continue; }
                if length(output) > MAX_PORT { errors.push(format!("{at}: output \"{output}\" is longer than {MAX_PORT} characters.")); }
                else if !is_typed_name(output) && !is_type_expression(output) { errors.push(format!("{at}: output \"{output}\" must be a TypeScript type or name: Type, for example Promise<boolean>.")); }
            }
        }
        // A file of types is not a module, with or without default values next to them: it does
        // nothing in the module graph. Its types belong to the module that uses them. A module of
        // constants alone (configuration) is allowed.
        let declares_types = m.interfaces.iter().any(|i| i.kind == InterfaceKind::Type);
        let only_server = !m.interfaces.is_empty() && m.interfaces.iter().all(|i| i.kind == InterfaceKind::Server);
        if (declares_types || only_server) && !m.interfaces.iter().any(|i| i.kind.acts()) {
            errors.push(format!("\"{label}\" declares types, constants or server requests but no component, hook, function or store, so it is not a module. Put its types and file into the module that uses them."));
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
    // An interface belongs to one module; a second module that lists it is re-exporting it.
    let planned: Vec<&Module> = modules.iter().filter(|m| m.kind != Kind::Existing).collect();
    let mut owner: HashMap<&str, &Module> = HashMap::new();
    for m in &planned {
        for interface in m.interfaces.iter().filter(|i| i.kind != InterfaceKind::Server && !i.name.trim().is_empty()) {
            if let Some(first) = owner.insert(interface.name.as_str(), m) {
                if first.id != m.id { errors.push(format!("Interface \"{}\" is listed by both \"{}\" and \"{}\". It belongs to one module; the other imports it.", interface.name, first.name, m.name)); }
            }
        }
    }
    // A module whose every acting interface has exactly the signature of an interface of a module
    // it imports from only passes that module through. That is the slice's index, not a module.
    let signature = |i: &Interface| (normalized(&i.inputs), normalized(&i.outputs));
    for m in &planned {
        let own: Vec<&Interface> = m.interfaces.iter().filter(|i| i.kind.acts()).collect();
        let imported: Vec<(&Module, &Interface)> = graph.get(m.id.as_str()).into_iter().flatten()
            .filter_map(|id| by_id.get(id).copied())
            .flat_map(|dependency| dependency.interfaces.iter().filter(|i| i.kind.acts()).map(move |i| (dependency, i)))
            .collect();
        let twins: Vec<(&Interface, &(&Module, &Interface))> = own.iter()
            .filter_map(|i| imported.iter().find(|(_, other)| !other.outputs.is_empty() && signature(other) == signature(i)).map(|twin| (*i, twin)))
            .collect();
        if !own.is_empty() && twins.len() == own.len() {
            let (interface, (source, original)) = twins[0];
            errors.push(format!("\"{}\" only passes through \"{}\": {} has the same inputs and outputs as {}. A re-export is the slice's index, not a module.", m.name, source.name, interface.name, original.name));
        }
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
        "id":m.id,"name":m.name,"kind":m.kind.name(),"path":m.path(),"description":m.responsibility,
        "interfaces":m.interfaces.iter().map(|i| json!({"name":i.name,"kind":i.kind.name(),"condition":i.condition,"result":i.result,"inputs":i.inputs,"outputs":i.outputs})).collect::<Vec<_>>(),
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
        let deps: Vec<Value> = dependencies.into_iter().map(|(from, to)| json!({"from":from,"to":to})).collect();
        serde_json::from_value(json!({"summary":"s","modules":modules,"dependencies":deps})).expect("fixture parses")
    }
    /// Validates against a fake repository: `paths` are placements, and every placement holds a.ts.
    fn check(p: &Proposal, paths: &[&str]) -> Vec<String> {
        let paths: HashSet<String> = paths.iter().map(|p| (*p).to_owned()).collect();
        let files: HashSet<String> = paths.iter().map(|p| format!("{p}/a.ts")).collect();
        validate(p, &Disk { paths: &paths, file_exists: &|file| files.contains(file) })
    }
    /// Validates one new module whose only interface is the given one.
    fn check_interface(interface: Value) -> Vec<String> {
        let mut m = module("a", "new", "features", Some("x"), "ui");
        m["interfaces"] = json!([interface]);
        check(&proposal(vec![m], vec![]), &[])
    }
    fn has(errors: &[String], part: &str) -> bool { errors.iter().any(|e| e.contains(part)) }
    const NONE: Vec<String> = Vec::new();

    #[test]
    fn accepts_a_well_formed_plan() {
        let p = proposal(vec![
            module("ui", "new", "features", Some("cancel-job"), "ui"),
            module("model", "modify", "features", Some("plan-job"), "model"),
            module("api", "existing", "shared", None, "api"),
        ], vec![("ui", "api"), ("model", "api")]);
        assert_eq!(check(&p, &["features/plan-job/model", "shared/api"]), NONE);
    }

    #[test]
    fn the_schema_cannot_express_code_outside_the_fsd_layers() {
        let outside = json!({"summary":"s","dependencies":[],"modules":[module("srv", "new", "server", None, "src")]});
        assert!(serde_json::from_value::<Proposal>(outside).is_err());
    }

    #[test]
    fn rejects_unknown_fields_and_unknown_kinds() {
        let mut m = module("a", "new", "features", Some("x"), "ui");
        m["behaviors"] = json!(["free prose"]);
        assert!(serde_json::from_value::<Proposal>(json!({"summary":"s","dependencies":[],"modules":[m]})).is_err());
        let mut k = module("a", "new", "features", Some("x"), "ui");
        k["interfaces"] = json!([interface("x", "service")]);
        assert!(serde_json::from_value::<Proposal>(json!({"summary":"s","dependencies":[],"modules":[k]})).is_err());
    }

    #[test]
    fn rejects_claims_about_modules_that_are_not_on_disk() {
        let p = proposal(vec![module("m", "modify", "features", Some("ghost"), "model")], vec![]);
        assert!(has(&check(&p, &[]), "does not exist in the repository"));
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
    fn rejects_missing_interfaces_and_dangling_dependencies() {
        let mut m = module("a", "new", "features", Some("x"), "ui");
        m["interfaces"] = json!([]);
        let errors = check(&proposal(vec![m], vec![("a", "nowhere")]), &[]);
        assert!(has(&errors, "exposes no interface") && has(&errors, "not in the plan"), "{errors:?}");
    }

    #[test]
    fn an_interface_needs_a_unique_name_but_not_inputs_or_outputs() {
        let mut bare = interface("Thing", "component");
        bare["inputs"] = json!([]);
        bare["outputs"] = json!([]);
        assert_eq!(check_interface(bare), NONE);
        let mut bad = module("b", "new", "features", Some("x"), "ui");
        bad["interfaces"] = json!([interface("", "hook"), interface("A", "hook"), interface("A", "hook")]);
        let errors = check(&proposal(vec![bad], vec![]), &[]);
        assert!(has(&errors, "without a name") && has(&errors, "more than once"), "{errors:?}");
    }

    #[test]
    fn behavior_is_a_condition_and_a_result() {
        // Something that acts must state its result; the condition may be empty.
        let mut always = interface("format", "function");
        always["condition"] = json!("");
        assert_eq!(check_interface(always), NONE);
        let mut silent = interface("cancelJob", "function");
        silent["result"] = json!(" ");
        assert!(has(&check_interface(silent), "states no result"));
        // A type states no behavior, but a file of types is still not a module.
        let mut shape = interface("Job", "type");
        shape["condition"] = json!("");
        shape["result"] = json!("");
        let mut m = module("a", "new", "features", Some("x"), "model");
        m["interfaces"] = json!([shape, interface("cancelJob", "function")]);
        assert_eq!(check(&proposal(vec![m], vec![]), &[]), NONE);
        // Existing modules are only referenced, so their behavior may be left out.
        let mut used = interface("apiRequest", "function");
        used["result"] = json!("");
        let mut api = module("api", "existing", "shared", None, "api");
        api["interfaces"] = json!([used]);
        let p = proposal(vec![module("a", "new", "features", Some("x"), "ui"), api], vec![("a", "api")]);
        assert_eq!(check(&p, &["shared/api"]), NONE);
    }

    #[test]
    fn prose_is_limited_in_length_and_to_one_sentence() {
        let mut m = module("a", "new", "features", Some("x"), "ui");
        m["name"] = json!("n".repeat(MAX_NAME + 1));
        m["responsibility"] = json!("Does one thing. And then another thing.");
        let mut long = interface("cancelJob", "function");
        long["condition"] = json!("w".repeat(MAX_CONDITION + 1));
        long["result"] = json!("가".repeat(MAX_RESULT + 1));
        m["interfaces"] = json!([long]);
        let mut p = proposal(vec![m], vec![]);
        p.summary = "s".repeat(MAX_SUMMARY + 1);
        let errors = check(&p, &[]);
        for part in ["Module name", "must be one sentence", "condition has", "result has", "The summary has"] {
            assert!(has(&errors, part), "missing {part:?} in {errors:?}");
        }
        // Limits count characters, not bytes: 100 Hangul syllables fit.
        let mut korean = interface("cancelJob", "function");
        korean["result"] = json!("가".repeat(MAX_RESULT));
        assert_eq!(check_interface(korean), NONE);
    }

    #[test]
    fn lists_are_limited_in_size() {
        let mut m = module("a", "new", "features", Some("x"), "ui");
        m["interfaces"] = Value::Array((0..=MAX_INTERFACES).map(|i| interface(&format!("f{i}"), "function")).collect());
        m["files"] = Value::Array((0..=MAX_FILES).map(|i| json!(format!("f{i}.ts"))).collect());
        let errors = check(&proposal(vec![m], vec![]), &[]);
        assert!(has(&errors, "interfaces; at most") && has(&errors, "files; at most"), "{errors:?}");
        let mut wide = interface("f", "function");
        wide["inputs"] = Value::Array((0..=MAX_PORTS).map(|i| json!(format!("a{i}: string"))).collect());
        assert!(has(&check_interface(wide), "inputs or outputs"));
    }

    #[test]
    fn inputs_and_outputs_are_names_and_types_not_descriptions() {
        let with = |inputs: Value, outputs: Value| {
            let mut i = interface("f", "function");
            i["inputs"] = inputs;
            i["outputs"] = outputs;
            check_interface(i)
        };
        // Accepted: typed names, optional names, and the usual type syntax.
        assert_eq!(with(json!(["id: string", "target?: string | null", "options: { retry: boolean; times: number }", "onDone: (job: Job) => void"]),
            json!(["Promise<boolean>", "job: JobView | null", "Record<string, readonly Job[]>", "keyof typeof STATUS", "'queued' | 'running'", "T extends Job ? string : never"])), NONE);
        // Rejected: descriptions, missing types, prose in any language, unbalanced brackets.
        assert!(has(&with(json!(["the job id"]), json!([])), "must be written name: Type"));
        assert!(has(&with(json!(["id"]), json!([])), "must be written name: Type"));
        assert!(has(&with(json!(["id: 작업 ID 경로 매개변수"]), json!([])), "must be written name: Type"));
        assert!(has(&with(json!(["id: the id of the job"]), json!([])), "must be written name: Type"));
        assert!(has(&with(json!([]), json!(["작업 표시 및 취소 제어에 필요한 상태 구조"])), "must be a TypeScript type"));
        assert!(has(&with(json!([]), json!(["the current job state"])), "must be a TypeScript type"));
        assert!(has(&with(json!([]), json!(["Promise<boolean"])), "must be a TypeScript type"));
    }

    #[test]
    fn a_file_of_types_is_not_a_module() {
        let mut types = module("contract", "new", "features", Some("x"), "model");
        types["interfaces"] = json!([interface("RunContract", "type"), interface("RunResult", "type")]);
        assert!(has(&check(&proposal(vec![types], vec![]), &[]), "so it is not a module"));
        // Types next to something usable are fine: they are part of that module's interface.
        let mut with_types = module("model", "new", "features", Some("x"), "model");
        with_types["interfaces"] = json!([interface("RunContract", "type"), interface("runSentence", "function")]);
        assert_eq!(check(&proposal(vec![with_types], vec![]), &[]), NONE);
    }

    #[test]
    fn types_with_a_default_value_are_still_not_a_module() {
        // The shape that slipped through: three types and one constant holding an empty value.
        let mut contract = module("contract", "new", "features", Some("x"), "model");
        let mut empty = interface("EMPTY_RESULT", "constant");
        empty["condition"] = json!("");
        empty["result"] = json!("");
        contract["interfaces"] = json!([empty.clone(), interface("RunInput", "type"), interface("RunResult", "type")]);
        assert!(has(&check(&proposal(vec![contract], vec![]), &[]), "so it is not a module"));
        // Constants alone are configuration, which is a module.
        let mut config = module("config", "new", "shared", None, "config");
        config["interfaces"] = json!([empty]);
        assert_eq!(check(&proposal(vec![config], vec![]), &[]), NONE);
    }

    #[test]
    fn an_output_that_repeats_the_name_is_rejected() {
        let mut shape = interface("RunInput", "type");
        shape["outputs"] = json!(["RunInput"]);
        let mut m = module("a", "new", "features", Some("x"), "model");
        m["interfaces"] = json!([shape, interface("run", "function")]);
        assert!(has(&check(&proposal(vec![m], vec![]), &[]), "only repeats the name"));
        let mut value = interface("Sentence", "constant");
        value["outputs"] = json!(["typeof Sentence"]);
        assert!(has(&check_interface(value), "only repeats the name"));
    }

    #[test]
    fn an_interface_has_one_owner_and_a_pass_through_is_not_a_module() {
        // "api" re-exports the hook of "hook" under another name, and lists its type again.
        let mut hook = module("hook", "new", "features", Some("x"), "lib");
        hook["interfaces"] = json!([interface("useRuntime", "hook"), interface("RunResult", "type")]);
        let mut api = module("api", "new", "features", Some("x"), "public-api");
        let mut reexport = interface("useSentences", "hook");
        reexport["outputs"] = interface("useRuntime", "hook")["outputs"].clone();
        api["interfaces"] = json!([reexport, interface("RunResult", "type")]);
        let errors = check(&proposal(vec![hook.clone(), api], vec![("api", "hook")]), &[]);
        assert!(has(&errors, "is listed by both"), "{errors:?}");
        assert!(has(&errors, "only passes through"), "{errors:?}");
        // A module that adds something of its own next to a wrapper is a module.
        let mut different = interface("useSentences", "hook");
        different["outputs"] = json!(["SlotContent"]);
        let mut widget = module("widget", "new", "widgets", Some("editor"), "model");
        widget["interfaces"] = json!([different]);
        assert_eq!(check(&proposal(vec![hook, widget], vec![("widget", "hook")]), &[]), NONE);
    }

    #[test]
    fn server_requests_are_interfaces_and_pass_through_shared_api() {
        let request = interface("POST /api/jobs/{id}/cancel", "server");
        let mut model = module("model", "new", "features", Some("cancel"), "model");
        model["interfaces"] = json!([interface("cancelJob", "function"), request]);
        let api = module("api", "existing", "shared", None, "api");
        let lib = module("lib", "existing", "shared", None, "lib");
        // Directly through shared/api, or indirectly through another module: accepted.
        assert_eq!(check(&proposal(vec![model.clone(), api.clone()], vec![("model", "api")]), &["shared/api"]), NONE);
        let mut ui = module("ui", "new", "features", Some("cancel"), "ui");
        ui["interfaces"] = json!([interface("CancelButton", "component"), interface("GET /api/jobs/{id}", "server")]);
        let plain_model = module("model", "new", "features", Some("cancel"), "model");
        assert_eq!(check(&proposal(vec![ui, plain_model, api.clone()], vec![("ui", "model"), ("model", "api")]), &["shared/api"]), NONE);
        // Bypassing shared/api: rejected.
        let errors = check(&proposal(vec![model.clone(), lib], vec![("model", "lib")]), &["shared/lib"]);
        assert!(has(&errors, "does not depend, directly or indirectly, on a module in shared/api"), "{errors:?}");
        // A server interface that is not named as a request: rejected.
        let mut misnamed = model;
        misnamed["interfaces"] = json!([interface("cancelJob", "function"), interface("cancel the job", "server")]);
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
