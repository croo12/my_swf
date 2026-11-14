//! FSD-aware, conservative repository scanner.
//!
//! Observes the filesystem layout and the import edges it can resolve; I/O contracts are NOT guessed.
//!
//! - The FSD source root is found inside the repository (monorepos keep it in e.g. `services/client/src`).
//! - Below a sliced layer, folders are grouping folders until one has an index file (the slice's
//!   public API). That folder is the slice; its sub-folders are the segments.
//! - An import of a slice's public API is traced through the index re-exports to the segment that
//!   declares the imported name.
mod context;
mod files;
mod imports;
mod modules;
mod scan;

use files::*;
use imports::*;
pub(crate) use modules::interface_kind;
pub use modules::slice_modules;
pub use scan::scan;

use regex::Regex;
use serde_json::{Value, json};
use std::collections::BTreeMap;
use std::{
    collections::{BTreeSet, HashMap, HashSet},
    env, fs,
    path::{Component, Path, PathBuf},
};
use walkdir::WalkDir;

type PublicApi = HashMap<String, BTreeSet<(String, String)>>;
type ImportFacts = BTreeSet<(String, String, String)>;

const LAYERS: &[&str] = &["app", "pages", "widgets", "features", "entities", "shared"];
const SOURCE_EXTS: &[&str] = &["ts", "tsx", "js", "jsx", "mts", "mjs"];
/// Folder names that mark their parent as a slice even when it has no index file.
const STANDARD_SEGMENTS: &[&str] = &["ui", "model", "api", "lib", "config"];
const IGNORED_DIRS: &[&str] = &[
    "node_modules",
    "dist",
    "build",
    "coverage",
    "target",
    "storybook-static",
    "__tests__",
    "__snapshots__",
    "__mocks__",
    "__fixtures__",
];
/// Segment name used for a slice that keeps its files directly in the slice folder.
const ROOT_SEGMENT: &str = "index";
/// FSD cross-import notation: `entities/a/@x/b.ts` is the API that entity a exposes to entity b.
const CROSS_API_DIR: &str = "@x";
const MAX_ROOT_DEPTH: usize = 6;
const MAX_GROUP_DEPTH: usize = 6;
const MAX_REEXPORT_DEPTH: usize = 10;
/// How many untraced imports are listed in the analysis result, so the cause can be looked up.
const MAX_UNRESOLVED_SAMPLES: usize = 50;
/// Stands for "everything", where an import does not name what it takes (`import * as`, side-effect imports).
pub const ALL_NAMES: &str = "*";

struct Slice {
    layer: &'static str,
    /// Path parts between the layer and the segments, e.g. ["home", "assignment"].
    rel: Vec<String>,
    dir: PathBuf,
    segments: Vec<String>,
}

/// Share the scanner's slice boundaries with the recursive repository browser.
pub(crate) fn source_layout(repo: &Path) -> Result<(PathBuf, Vec<String>), String> {
    let root = find_source_root(repo)?;
    let layout = Scan::discover(&root);
    let slices = layout
        .slices
        .iter()
        .map(|slice| format!("{}/{}", slice.layer, slice.rel.join("/")))
        .collect();
    Ok((root, slices))
}

enum Location {
    /// A segment node, identified by its plan path.
    Node(String),
    /// A public API of the slice: its index file (None) or an `@x` file. Which segment is meant
    /// depends on the imported names.
    Api(usize, Option<PathBuf>),
    Outside,
}

enum Names {
    All,
    List(Vec<String>),
}

struct Scan {
    root: PathBuf,
    slices: Vec<Slice>,
    slice_index: HashMap<(String, String), usize>,
    direct_segments: HashMap<&'static str, Vec<String>>,
}

struct Patterns {
    import_from: Regex,
    bare_import: Regex,
    export_star: Regex,
    export_list: Regex,
    export_decl: Regex,
}
