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
    /// Names of the interfaces of `to` that `from` uses. This is what the dependency is for.
    pub uses: Vec<String>,
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
                "type":"object","additionalProperties":false,"required":["from","to","uses"],
                "properties":{
                    "from":{"type":"string","description":"id of the module that imports."},
                    "to":{"type":"string","description":"id of the module it imports from."},
                    "uses":{"type":"array","items":{"type":"string"},"description":"Names of the interfaces of the imported module that the importing module uses. At least one."}
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
/// Who wrote the plan. An agent may mention an existing module only for what its planned work
/// uses. A person draws the plan starting from the existing structure, so existing modules keep
/// their real interfaces and the real dependencies between them.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Author { Agent, Person }

/// Turns the plan document the client edits back into a proposal, so a plan drawn by a person is
/// checked by the same rules. Code the server added around the plan is left out.
pub fn proposal_from_plan(plan: &Value) -> Result<Proposal, String> {
    let observed = |value: &Value| value.get("observed").and_then(Value::as_bool).unwrap_or(false);
    let text = |value: &Value, key: &str| value.get(key).and_then(Value::as_str).unwrap_or("").trim().to_owned();
    let list = |value: &Value, key: &str| -> Vec<Value> { value.get(key).and_then(Value::as_array).cloned().unwrap_or_default() };
    let nodes: Vec<Value> = list(plan, "nodes").into_iter().filter(|n| !observed(n)).collect();
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
    let dependencies: Vec<Value> = list(plan, "edges").into_iter()
        .filter(|e| !observed(e) && ids.contains(&text(e, "from")) && ids.contains(&text(e, "to")))
        .map(|e| json!({"from":text(&e, "from"),"to":text(&e, "to"),"uses":list(&e, "uses")})).collect();
    serde_json::from_value(json!({"summary":text(plan, "summary"),"modules":modules,"dependencies":dependencies}))
        .map_err(|e| format!("The plan cannot be read as modules and dependencies: {e}"))
}

pub fn validate(proposal: &Proposal, disk: &Disk, author: Author) -> Vec<String> {
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
        // What an unchanged module is for is already in its code; the plan need not restate it.
        if m.responsibility.trim().is_empty() && !(m.kind == Kind::Existing && author == Author::Person) { errors.push(format!("Module \"{label}\" has no responsibility.")); }
        if length(&m.name) > MAX_NAME { errors.push(format!("Module name \"{label}\" has {} characters; at most {MAX_NAME} are allowed.", length(&m.name))); }
        if length(&m.responsibility) > MAX_RESPONSIBILITY { errors.push(format!("Module \"{label}\": the responsibility has {} characters; at most {MAX_RESPONSIBILITY} are allowed.", length(&m.responsibility))); }
        if !is_one_sentence(&m.responsibility) { errors.push(format!("Module \"{label}\": the responsibility must be one sentence.")); }
        // Code a person brought in unchanged is a fact about the repository, not a design to judge.
        let as_found = m.kind == Kind::Existing && author == Author::Person;
        if m.interfaces.len() > MAX_INTERFACES && !as_found { errors.push(format!("Module \"{label}\" has {} interfaces; at most {MAX_INTERFACES} are allowed. Split the module.", m.interfaces.len())); }
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
        if (declares_types || only_server) && !m.interfaces.iter().any(|i| i.kind.acts()) && !as_found {
            errors.push(format!("\"{label}\" declares types, constants or server requests but no component, hook, function or store, so it is not a module. Put its types and file into the module that uses them."));
        }

        // Claims about the repository are checked against the repository.
        let on_disk = disk.paths.contains(&m.path());
        if m.kind != Kind::New && !on_disk { errors.push(format!("Module \"{label}\" is marked {} but {} does not exist in the repository.", m.kind.name(), m.path())); }
    }

    let mut seen = HashSet::new();
    let mut depended_on: HashSet<&str> = HashSet::new();
    // (module id, interface name) pairs that some dependency uses.
    let mut used: HashSet<(&str, &str)> = HashSet::new();
    let mut graph: HashMap<&str, Vec<&str>> = HashMap::new();
    for d in &proposal.dependencies {
        let (Some(from), Some(to)) = (by_id.get(d.from.as_str()), by_id.get(d.to.as_str())) else {
            errors.push(format!("Dependency {} -> {} refers to a module that is not in the plan.", d.from, d.to));
            continue;
        };
        if d.from == d.to { errors.push(format!("Module \"{}\" depends on itself.", from.name)); continue; }
        if !seen.insert((d.from.as_str(), d.to.as_str())) { errors.push(format!("Dependency \"{}\" -> \"{}\" is listed more than once.", from.name, to.name)); continue; }
        if from.kind == Kind::Existing && author == Author::Agent { errors.push(format!("Existing module \"{}\" is unchanged, so the plan must not give it dependencies.", from.name)); }
        // FSD import rule: only downward, or within one slice (app and shared count as one slice each).
        let same_slice = from.layer == to.layer && from.slice == to.slice;
        if !(from.layer.rank() < to.layer.rank() || same_slice) {
            errors.push(format!("\"{}\" ({}) may not import \"{}\" ({}): imports go to a lower layer or stay inside one slice.", from.name, from.path(), to.name, to.path()));
        }
        // A dependency exists for the interfaces it uses, and those must be interfaces the target
        // really offers. Server requests face the server and cannot be used by another module.
        if d.uses.is_empty() { errors.push(format!("Dependency \"{}\" -> \"{}\" names no interface it uses.", from.name, to.name)); }
        let mut named = HashSet::new();
        for name in &d.uses {
            if !named.insert(name.as_str()) { errors.push(format!("Dependency \"{}\" -> \"{}\" lists \"{name}\" more than once.", from.name, to.name)); continue; }
            match to.interfaces.iter().find(|i| &i.name == name) {
                None => errors.push(format!("Dependency \"{}\" -> \"{}\" uses \"{name}\", which \"{}\" does not offer.", from.name, to.name, to.name)),
                Some(i) if i.kind == InterfaceKind::Server => errors.push(format!("Dependency \"{}\" -> \"{}\" uses the server request \"{name}\"; a module can only use what another module offers.", from.name, to.name)),
                Some(_) => { used.insert((d.to.as_str(), name.as_str())); }
            }
        }
        depended_on.insert(d.to.as_str());
        graph.entry(d.from.as_str()).or_default().push(d.to.as_str());
    }
    for m in modules.iter().filter(|m| m.kind == Kind::Existing && author == Author::Agent) {
        if !depended_on.contains(m.id.as_str()) { errors.push(format!("Existing module \"{}\" is in the plan but nothing depends on it.", m.name)); }
    }
    // An existing module is in the plan only for what the planned work uses, so every interface it
    // lists must be used by a dependency.
    for m in modules.iter().filter(|m| m.kind == Kind::Existing && author == Author::Agent) {
        // Server requests of an existing module are its own business with the server.
        for interface in m.interfaces.iter().filter(|i| i.kind != InterfaceKind::Server) {
            if !used.contains(&(m.id.as_str(), interface.name.as_str())) {
                errors.push(format!("Existing module \"{}\" lists \"{}\", which no dependency uses. List only the interfaces that are used.", m.name, interface.name));
            }
        }
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

/// One import that exists in the repository.
#[derive(Debug, Clone)]
pub struct ObservedImport {
    /// Placement of the importing file, and the file itself relative to the source root.
    pub from: String,
    pub file: String,
    /// Placement that provides the name, and the file that declares it (empty when unknown).
    pub to: String,
    pub provider: String,
    /// The imported name; "*" where the import does not name what it takes.
    pub name: String,
}

/// Reads the imports out of a repository scan.
pub fn observed_imports(scan: &Value) -> Vec<ObservedImport> {
    let text = |value: &Value, key: &str| value.get(key).and_then(Value::as_str).unwrap_or("").to_owned();
    // Imports inside one placement connect files of the same segment; the plan's surroundings
    // are about what lies outside a placement.
    scan.get("records").and_then(Value::as_array).into_iter().flatten()
        .map(|r| ObservedImport { from: text(r, "from"), file: text(r, "file"), to: text(r, "to"), provider: text(r, "provider"), name: text(r, "name") })
        .filter(|import| import.from != import.to).collect()
}

const UNNAMED_IMPORT: &str = "*";

fn without_extension(path: &str) -> &str {
    match path.rsplit_once('.') {
        Some((stem, extension)) if SOURCE_EXTENSIONS.contains(&extension) => stem,
        _ => path,
    }
}

/// Whether `file` (relative to the source root, possibly without an extension, possibly a folder)
/// is one of the module's files.
fn owns(module: &Module, file: &str) -> bool {
    if file.is_empty() { return false; }
    let file = without_extension(file);
    let path = module.path();
    module.files.iter().any(|own| {
        let inside = format!("{path}/{}", without_extension(own));
        // A segment can be a single file that sits next to the segment name.
        let beside = (without_extension(own) == module.segment).then(|| path.clone());
        [Some(inside), beside].into_iter().flatten().any(|own| own == file || own == format!("{file}/index") || own.starts_with(&format!("{file}/")))
    })
}

/// The code around the plan: files the plan does not mention that import from a planned module, or
/// that a changed module imports from. These come from the repository's imports, not from the
/// proposal, so they show what the planned change is really connected to.
///
/// An import belongs to a planned module if the module lists the imported name as an interface or
/// owns the file that declares it; an importing file belongs to the module that lists it. Imports
/// that touch a planned placement but none of the planned modules' files are left out.
pub fn surroundings(proposal: &Proposal, observed: &[ObservedImport]) -> (Vec<Value>, Vec<Value>) {
    let lists = |m: &Module, name: &str| name != UNNAMED_IMPORT && m.interfaces.iter().any(|i| i.name == name);
    // A name may move between segments of one slice; app and shared have no slices.
    let same_slice = |m: &Module, path: &str| m.path() == path
        || m.slice.as_ref().is_some_and(|slice| m.layer.has_slices() && path.starts_with(&format!("{}/{slice}/", m.layer.name())));
    let context_id = |path: &str| format!("observed-{}", path.replace('/', "--"));

    let mut context: Vec<String> = vec![];
    let mut order: Vec<(String, String)> = vec![];
    let mut uses: HashMap<(String, String), Vec<String>> = HashMap::new();
    let mut link = |from: String, to: String, name: &str, context_path: Option<&str>| {
        if let Some(new) = context_path.filter(|new| !context.iter().any(|path| path == new)) { context.push(new.to_owned()); }
        let key = (from, to);
        if !uses.contains_key(&key) { order.push(key.clone()); }
        let list = uses.entry(key).or_default();
        if name != UNNAMED_IMPORT && !list.iter().any(|listed| listed == name) { list.push(name.to_owned()); }
    };

    for import in observed {
        let importer = proposal.modules.iter().find(|m| m.path() == import.from && owns(m, &import.file));
        let planned_provider = proposal.modules.iter().filter(|m| m.kind != Kind::Existing)
            .find(|m| same_slice(m, &import.to) && lists(m, &import.name))
            .or_else(|| proposal.modules.iter().find(|m| m.kind == Kind::Modify && m.path() == import.to && owns(m, &import.provider)));
        match (importer, planned_provider) {
            // Unplanned code that uses a planned module.
            (None, Some(provider)) => link(context_id(&import.from), provider.id.clone(), &import.name, Some(&import.from)),
            // Unplanned code that a changed module uses. What the plan already lists is not repeated.
            (Some(m), None) if m.kind == Kind::Modify => {
                let listed = proposal.modules.iter().any(|other| other.path() == import.to && (lists(other, &import.name) || owns(other, &import.provider)));
                if listed { continue; }
                // If the plan has exactly one module at that placement, the import is of that
                // module: show it there instead of a second card for the same place, unless the
                // plan already states a dependency between the two.
                let there: Vec<&Module> = proposal.modules.iter().filter(|other| other.path() == import.to).collect();
                match there.as_slice() {
                    [only] => if !proposal.dependencies.iter().any(|d| d.from == m.id && d.to == only.id) { link(m.id.clone(), only.id.clone(), &import.name, None); },
                    _ => link(m.id.clone(), context_id(&import.to), &import.name, Some(&import.to)),
                }
            }
            _ => {}
        }
    }

    let nodes = context.iter().map(|path| json!({
        "id":context_id(path),"kind":Kind::Existing.name(),"path":path,"observed":true,
        "description":"계획에 없는 기존 코드. 저장소의 import로 확인한 연결입니다.","interfaces":[],"files":[]
    })).collect();
    let edges = order.iter().enumerate().map(|(i, key)| json!({
        "id":format!("observed-{}",i + 1),"from":key.0,"to":key.1,"type":"imports","uses":uses[key],"observed":true
    })).collect();
    (nodes, edges)
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
    let mut edges: Vec<Value> = proposal.dependencies.iter().enumerate().map(|(i, d)| json!({
        "id":format!("dep-{}",i + 1),"from":d.from,"to":d.to,"type":"imports","uses":d.uses
    })).collect();
    nodes.extend(context_nodes);
    edges.extend(context_edges);
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
        // By default a dependency uses every interface its target lists.
        let deps: Vec<Value> = dependencies.into_iter().map(|(from, to)| {
            let uses: Vec<Value> = modules.iter().filter(|m| m["id"] == to)
                .flat_map(|m| m["interfaces"].as_array().cloned().unwrap_or_default())
                .filter(|i| i["kind"] != "server").map(|i| i["name"].clone()).collect();
            json!({"from":from,"to":to,"uses":uses})
        }).collect();
        serde_json::from_value(json!({"summary":"s","modules":modules,"dependencies":deps})).expect("fixture parses")
    }
    fn import(from: &str, file: &str, to: &str, provider: &str, name: &str) -> ObservedImport {
        ObservedImport { from: from.to_owned(), file: file.to_owned(), to: to.to_owned(), provider: provider.to_owned(), name: name.to_owned() }
    }

    #[test]
    fn plan_shows_the_unplanned_code_around_changed_modules() {
        let p = proposal(vec![module("filter", "modify", "features", Some("variant-filter"), "model")], vec![]);
        let observed = vec![
            // Uses an interface the changed module lists.
            import("widgets/table/ui", "widgets/table/ui/Table.tsx", "features/variant-filter/model", "features/variant-filter/model/other.ts", "run_filter"),
            // Uses something declared in the changed module's file.
            import("pages/sample/ui", "pages/sample/ui/Page.tsx", "features/variant-filter/model", "features/variant-filter/model/a.ts", "FilterState"),
            // Uses another file of the same segment, which the plan does not touch.
            import("pages/other/ui", "pages/other/ui/Other.tsx", "features/variant-filter/model", "features/variant-filter/model/unrelated.ts", "unrelated"),
            // The changed module's file imports unplanned code; another file of the segment does too.
            import("features/variant-filter/model", "features/variant-filter/model/a.ts", "shared/lib", "shared/lib/date.ts", "formatDate"),
            import("features/variant-filter/model", "features/variant-filter/model/unrelated.ts", "shared/ui", "shared/ui/Button.tsx", "Button"),
        ];
        let plan = to_plan(&p, &observed);
        let ids = |key: &str| -> Vec<String> { plan[key].as_array().unwrap().iter().filter(|v| v["observed"] == true).map(|v| v["id"].as_str().unwrap().to_owned()).collect() };
        assert_eq!(ids("nodes"), ["observed-widgets--table--ui", "observed-pages--sample--ui", "observed-shared--lib"]);
        let edges: Vec<(String, String, String)> = plan["edges"].as_array().unwrap().iter()
            .map(|e| (e["from"].as_str().unwrap().to_owned(), e["to"].as_str().unwrap().to_owned(), e["uses"][0].as_str().unwrap_or("").to_owned())).collect();
        assert_eq!(edges, [
            ("observed-widgets--table--ui".to_owned(), "filter".to_owned(), "run_filter".to_owned()),
            ("observed-pages--sample--ui".to_owned(), "filter".to_owned(), "FilterState".to_owned()),
            ("filter".to_owned(), "observed-shared--lib".to_owned(), "formatDate".to_owned()),
        ]);
    }

    /// Validates against a fake repository: `paths` are placements, and every placement holds a.ts.
    fn check(p: &Proposal, paths: &[&str]) -> Vec<String> {
        let paths: HashSet<String> = paths.iter().map(|p| (*p).to_owned()).collect();
        let files: HashSet<String> = paths.iter().map(|p| format!("{p}/a.ts")).collect();
        validate(p, &Disk { paths: &paths, file_exists: &|file| files.contains(file) }, Author::Agent)
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
    fn a_dependency_names_the_interfaces_it_uses() {
        let mut api = module("api", "existing", "shared", None, "api");
        api["interfaces"] = json!([interface("apiRequest", "function"), interface("errorMessage", "function"), interface("GET /api/status", "server")]);
        let model = module("model", "new", "features", Some("x"), "model");
        let with = |uses: Value| {
            let mut p: Value = json!({"summary":"s","modules":[model.clone(), api.clone()],"dependencies":[{"from":"model","to":"api","uses":uses}]});
            // errorMessage is only listed when the case under test uses it.
            if !p["dependencies"][0]["uses"].as_array().is_some_and(|u| u.contains(&json!("errorMessage"))) {
                p["modules"][1]["interfaces"] = json!([interface("apiRequest", "function"), interface("GET /api/status", "server")]);
            }
            check(&serde_json::from_value(p).expect("fixture parses"), &["shared/api"])
        };
        assert_eq!(with(json!(["apiRequest", "errorMessage"])), NONE);
        assert!(has(&with(json!([])), "names no interface it uses"));
        assert!(has(&with(json!(["apiRequest", "apiRequest"])), "more than once"));
        assert!(has(&with(json!(["apiRequest", "fetchEverything"])), "does not offer"));
        assert!(has(&with(json!(["apiRequest", "GET /api/status"])), "uses the server request"));
        // An existing module may list only what is used.
        let p = json!({"summary":"s","modules":[model, module("api", "existing", "shared", None, "api")],"dependencies":[{"from":"model","to":"api","uses":["run_api"]}]});
        let mut extra = p.clone();
        extra["modules"][1]["interfaces"] = json!([interface("run_api", "function"), interface("unusedHelper", "function")]);
        assert_eq!(check(&serde_json::from_value(p).expect("fixture parses"), &["shared/api"]), NONE);
        assert!(has(&check(&serde_json::from_value(extra).expect("fixture parses"), &["shared/api"]), "which no dependency uses"));
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
