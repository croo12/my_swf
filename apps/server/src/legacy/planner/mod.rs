//! The structured result of a planning run, and the static checks it must pass.
//!
//! The AI is not trusted to follow instructions about shape. Its answer is constrained twice:
//! 1. `output_schema()` is handed to the agent as a JSON Schema, so it can only answer in this shape.
//! 2. `validate()` checks the parsed answer against the rules below and against what exists on
//!    disk. A proposal with any violation is rejected as a whole; nothing is repaired or guessed.
mod convert;
mod observed;
mod schema;
mod syntax;
#[cfg(test)]
mod tests;
mod validation;

pub use convert::{proposal_from_plan, to_plan};
pub use observed::{observed_imports, surroundings, ObservedImport};
pub use schema::output_schema;
use serde::Deserialize;
use std::collections::HashSet;
pub use validation::validate;

pub const MAX_MODULES: usize = 60;
/// The segment of the shared layer through which every request to the server passes.
const API_SEGMENT: &str = "api";
/// File types that can live in the FSD layers. A plan cannot place other code (for example Rust) there.
const SOURCE_EXTENSIONS: &[&str] = &[
    "ts", "tsx", "js", "jsx", "mts", "mjs", "css", "scss", "json",
];

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
const TYPE_KEYWORDS: &[&str] = &[
    "keyof", "typeof", "readonly", "extends", "infer", "in", "is", "new", "unique", "asserts",
    "abstract",
];

/// What the validator needs to know about the repository.
pub struct Disk<'a> {
    /// Module placements found by the scanner (layer/slice/segment).
    pub paths: &'a HashSet<String>,
    /// Whether a file exists, given its path relative to the FSD source root.
    pub file_exists: &'a dyn Fn(&str) -> bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Layer {
    App,
    Pages,
    Widgets,
    Features,
    Entities,
    Shared,
}

impl Layer {
    pub fn name(self) -> &'static str {
        match self {
            Layer::App => "app",
            Layer::Pages => "pages",
            Layer::Widgets => "widgets",
            Layer::Features => "features",
            Layer::Entities => "entities",
            Layer::Shared => "shared",
        }
    }
    /// Position from the top: a module may import only from layers with a higher rank.
    fn rank(self) -> usize {
        self as usize
    }
    /// app and shared hold segments directly; the other layers group them into slices.
    fn has_slices(self) -> bool {
        !matches!(self, Layer::App | Layer::Shared)
    }
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
    fn name(self) -> &'static str {
        match self {
            Kind::New => "new",
            Kind::Modify => "modify",
            Kind::Existing => "existing",
        }
    }
}

/// What kind of thing an interface is. `Server` is a request the module sends to the server: the
/// module's interface with the outside, as opposed to what it offers to other modules.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum InterfaceKind {
    Component,
    Hook,
    Function,
    Store,
    Constant,
    Type,
    Server,
}

impl InterfaceKind {
    fn name(self) -> &'static str {
        match self {
            InterfaceKind::Component => "component",
            InterfaceKind::Hook => "hook",
            InterfaceKind::Function => "function",
            InterfaceKind::Store => "store",
            InterfaceKind::Constant => "constant",
            InterfaceKind::Type => "type",
            InterfaceKind::Server => "server",
        }
    }
    /// Something that does work when another module uses it. A type describes a shape, a constant
    /// is a value, and a server request faces the server; none of them makes a module on its own.
    fn acts(self) -> bool {
        matches!(
            self,
            InterfaceKind::Component
                | InterfaceKind::Hook
                | InterfaceKind::Function
                | InterfaceKind::Store
        )
    }
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
            Some(slice) if self.layer.has_slices() => {
                format!("{}/{}/{}", self.layer.name(), slice, self.segment)
            }
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

/// Checks a proposal against the rules and against the repository.
/// Returns every violation found, so a rejected proposal can be understood in one read.
/// Who wrote the plan. An agent may mention an existing module only for what its planned work
/// uses. A person draws the plan starting from the existing structure, so existing modules keep
/// their real interfaces and the real dependencies between them.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Author {
    Agent,
    Person,
}
