//! FSD-aware, conservative repository scanner.
//!
//! Observes the filesystem layout and the import edges it can resolve; I/O contracts are NOT guessed.
//!
//! - The FSD source root is found inside the repository (monorepos keep it in e.g. `services/client/src`).
//! - Below a sliced layer, folders are grouping folders until one has an index file (the slice's
//!   public API). That folder is the slice; its sub-folders are the segments.
//! - An import of a slice's public API is traced through the index re-exports to the segment that
//!   declares the imported name.
use regex::Regex;
use std::collections::BTreeMap;
use serde_json::{json, Value};
use std::{
    collections::{BTreeSet, HashMap, HashSet},
    env, fs,
    path::{Component, Path, PathBuf},
};
use walkdir::WalkDir;

const LAYERS: &[&str] = &["app", "pages", "widgets", "features", "entities", "shared"];
const SOURCE_EXTS: &[&str] = &["ts", "tsx", "js", "jsx", "mts", "mjs"];
/// Folder names that mark their parent as a slice even when it has no index file.
const STANDARD_SEGMENTS: &[&str] = &["ui", "model", "api", "lib", "config"];
const IGNORED_DIRS: &[&str] = &[
    "node_modules", "dist", "build", "coverage", "target", "storybook-static",
    "__tests__", "__snapshots__", "__mocks__", "__fixtures__",
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

fn is_ignored_dir(name: &str) -> bool { name.starts_with('.') || IGNORED_DIRS.contains(&name) }
fn is_direct_layer(layer: &str) -> bool { layer == "app" || layer == "shared" }
fn is_source_file(path: &Path) -> bool {
    path.extension().and_then(|e| e.to_str()).is_some_and(|e| SOURCE_EXTS.contains(&e))
}
/// Tests and stories import widely for their own purposes; they are not part of the architecture.
fn is_test_file(path: &Path) -> bool {
    let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
    [".test.", ".spec.", ".stories."].iter().any(|marker| name.contains(marker))
}
fn child_dirs(dir: &Path) -> Vec<(String, PathBuf)> {
    let mut dirs: Vec<(String, PathBuf)> = fs::read_dir(dir).into_iter().flatten().flatten()
        .filter(|e| e.path().is_dir())
        .map(|e| (e.file_name().to_string_lossy().to_string(), e.path()))
        .filter(|(name, _)| !is_ignored_dir(name))
        .collect();
    dirs.sort();
    dirs
}
/// Segments of a slice or direct layer: its sub-folders, plus source files that sit directly in it
/// (`types.ts` is a segment just like `ui/`). The index file and the `@x` cross-import folder are
/// public API surface, not segments.
fn segment_names(dir: &Path) -> Vec<String> {
    let mut names: BTreeSet<String> = child_dirs(dir).into_iter().map(|(name, _)| name).filter(|name| name != CROSS_API_DIR).collect();
    for entry in fs::read_dir(dir).into_iter().flatten().flatten() {
        let path = entry.path();
        if !path.is_file() || !is_source_file(&path) || is_test_file(&path) { continue; }
        let stem = path.file_stem().unwrap_or_default().to_string_lossy().to_string();
        // "x.d.ts" has the stem "x.d"; declaration files are not segments.
        if stem != "index" && !stem.ends_with(".d") { names.insert(stem); }
    }
    names.into_iter().collect()
}
fn index_file(dir: &Path) -> Option<PathBuf> {
    SOURCE_EXTS.iter().map(|ext| dir.join(format!("index.{ext}"))).find(|p| p.is_file())
}
fn has_direct_source(dir: &Path) -> bool {
    fs::read_dir(dir).into_iter().flatten().flatten().any(|e| e.path().is_file() && is_source_file(&e.path()))
}
fn normalize(path: &Path) -> PathBuf {
    let mut out = PathBuf::new();
    for c in path.components() {
        match c {
            Component::ParentDir => { out.pop(); }
            Component::CurDir => {}
            other => out.push(other.as_os_str()),
        }
    }
    out
}
/// A path as the plan writes it: relative to the source root, with forward slashes.
fn relative(root: &Path, path: &Path) -> String {
    path.strip_prefix(root).unwrap_or(path).to_string_lossy().replace('\\', "/")
}
/// Resolves an extension-less module path to a file: `x.ts`, `x.tsx`, ..., or `x/index.ts`.
fn resolve_module(path: &Path) -> Option<PathBuf> {
    if path.is_file() { return Some(path.to_path_buf()); }
    // Declaration files can only be reached through an import, so they are tried here but never scanned.
    for ext in SOURCE_EXTS.iter().copied().chain(["d.ts"]) {
        let mut with_ext = path.as_os_str().to_owned();
        with_ext.push(format!(".{ext}"));
        let candidate = PathBuf::from(with_ext);
        if candidate.is_file() { return Some(candidate); }
    }
    if path.is_dir() { index_file(path) } else { None }
}

/// Finds the directory that holds the FSD layer folders. `FACTORY_SOURCE_ROOT` (relative to the
/// repository, or absolute) overrides the search.
fn find_source_root(repo: &Path) -> Result<PathBuf, String> {
    if let Ok(configured) = env::var("FACTORY_SOURCE_ROOT") {
        let root = repo.join(configured);
        return if root.is_dir() { Ok(root) } else { Err(format!("FACTORY_SOURCE_ROOT is not a directory: {}", root.display())) };
    }
    let layer_count = |dir: &Path| LAYERS.iter().filter(|layer| dir.join(layer).is_dir()).count();
    let mut best: Option<(usize, usize, PathBuf)> = None; // (layers found, depth, path)
    let walker = WalkDir::new(repo).follow_links(false).max_depth(MAX_ROOT_DEPTH).into_iter()
        .filter_entry(|e| e.depth() == 0 || !is_ignored_dir(&e.file_name().to_string_lossy()));
    for entry in walker.filter_map(Result::ok).filter(|e| e.file_type().is_dir()) {
        let score = layer_count(entry.path());
        // More layers wins; among equals the shallowest directory wins.
        if score >= 2 && best.as_ref().is_none_or(|(s, d, _)| score > *s || (score == *s && entry.depth() < *d)) {
            best = Some((score, entry.depth(), entry.path().to_path_buf()));
        }
    }
    best.map(|(_, _, path)| path).ok_or_else(|| format!(
        "No directory with FSD layer folders (app/pages/widgets/features/entities/shared) was found within {} levels of {}. Set FACTORY_SOURCE_ROOT to point at it.",
        MAX_ROOT_DEPTH, repo.display()
    ))
}

struct Slice {
    layer: &'static str,
    /// Path parts between the layer and the segments, e.g. ["home", "assignment"].
    rel: Vec<String>,
    dir: PathBuf,
    segments: Vec<String>,
}

enum Location {
    /// A segment node, identified by its plan path.
    Node(String),
    /// A public API of the slice: its index file (None) or an `@x` file. Which segment is meant
    /// depends on the imported names.
    Api(usize, Option<PathBuf>),
    Outside,
}

enum Names { All, List(Vec<String>) }

struct Scan {
    root: PathBuf,
    slices: Vec<Slice>,
    slice_index: HashMap<(String, String), usize>,
    direct_segments: HashMap<&'static str, Vec<String>>,
}

impl Scan {
    fn discover(root: &Path) -> Scan {
        let mut scan = Scan { root: root.to_path_buf(), slices: vec![], slice_index: HashMap::new(), direct_segments: HashMap::new() };
        for &layer in LAYERS {
            let layer_dir = root.join(layer);
            if !layer_dir.is_dir() { continue; }
            if is_direct_layer(layer) {
                scan.direct_segments.insert(layer, segment_names(&layer_dir));
            } else {
                for (name, dir) in child_dirs(&layer_dir) { scan.collect_slices(layer, vec![name], &dir); }
            }
        }
        for (i, slice) in scan.slices.iter().enumerate() {
            scan.slice_index.insert((slice.layer.to_owned(), slice.rel.join("/")), i);
        }
        scan
    }

    /// Walks down from a layer: a folder with an index file (or with files/standard segments of its
    /// own) is a slice; anything above it is a grouping folder.
    fn collect_slices(&mut self, layer: &'static str, rel: Vec<String>, dir: &Path) {
        let children = child_dirs(dir);
        let is_slice = index_file(dir).is_some()
            || has_direct_source(dir)
            || children.iter().any(|(name, _)| STANDARD_SEGMENTS.contains(&name.as_str()));
        if is_slice {
            let mut segments = segment_names(dir);
            if segments.is_empty() { segments.push(ROOT_SEGMENT.to_owned()); }
            self.slices.push(Slice { layer, rel, dir: dir.to_path_buf(), segments });
        } else if rel.len() < MAX_GROUP_DEPTH {
            for (name, child) in children {
                let mut child_rel = rel.clone();
                child_rel.push(name);
                self.collect_slices(layer, child_rel, &child);
            }
        }
    }

    fn node_paths(&self) -> Vec<String> {
        let mut paths = vec![];
        for &layer in LAYERS {
            if let Some(segments) = self.direct_segments.get(layer) {
                paths.extend(segments.iter().map(|segment| format!("{layer}/{segment}")));
            }
            for slice in self.slices.iter().filter(|s| s.layer == layer) {
                paths.extend(slice.segments.iter().map(|segment| format!("{layer}/{}/{segment}", slice.rel.join("/"))));
            }
        }
        paths
    }

    /// Maps a file or module path under the source root to the node (or slice) it belongs to.
    fn locate(&self, path: &Path) -> Location {
        let Ok(relative) = path.strip_prefix(&self.root) else { return Location::Outside; };
        let mut parts: Vec<String> = relative.components().map(|c| c.as_os_str().to_string_lossy().to_string()).collect();
        if let Some(last) = parts.last_mut() {
            if is_source_file(Path::new(last.as_str())) {
                *last = Path::new(last.as_str()).file_stem().unwrap_or_default().to_string_lossy().to_string();
            }
        }
        let Some(layer) = parts.first().map(String::as_str) else { return Location::Outside; };
        if let Some(segments) = self.direct_segments.get(layer) {
            return match parts.get(1) {
                Some(segment) if segments.contains(segment) => Location::Node(format!("{layer}/{segment}")),
                _ => Location::Outside,
            };
        }
        for end in 2..=parts.len() {
            let Some(&index) = self.slice_index.get(&(layer.to_owned(), parts[1..end].join("/"))) else { continue; };
            let slice = &self.slices[index];
            let node = |segment: &str| Location::Node(format!("{layer}/{}/{segment}", slice.rel.join("/")));
            return match parts.get(end).map(String::as_str) {
                None | Some("index") if slice.segments == [ROOT_SEGMENT] => node(ROOT_SEGMENT),
                None | Some("index") => Location::Api(index, None),
                Some(CROSS_API_DIR) => Location::Api(index, Some(path.to_path_buf())),
                Some(first) if slice.segments.iter().any(|s| s == first) => node(first),
                Some(_) if slice.segments == [ROOT_SEGMENT] => node(ROOT_SEGMENT),
                Some(_) => Location::Outside,
            };
        }
        Location::Outside
    }

    /// Turns an import specifier into a path under the source root, or None for npm packages.
    fn resolve_spec(&self, from_file: &Path, spec: &str) -> Option<PathBuf> {
        let logical = if spec.starts_with('.') {
            from_file.parent()?.join(spec)
        } else if let Some(rest) = ["@/", "~/", "src/"].iter().find_map(|prefix| spec.strip_prefix(prefix)) {
            self.root.join(rest)
        } else {
            // Layer aliases: "@features/x", "features/x".
            let bare = spec.strip_prefix('@').unwrap_or(spec);
            if !LAYERS.contains(&bare.split('/').next()?) { return None; }
            self.root.join(bare)
        };
        let path = normalize(&logical);
        path.starts_with(&self.root).then_some(path)
    }
}

struct Patterns {
    import_from: Regex,
    bare_import: Regex,
    export_star: Regex,
    export_list: Regex,
    export_decl: Regex,
}

impl Patterns {
    fn new() -> Result<Patterns, String> {
        let re = |pattern: &str| Regex::new(pattern).map_err(|e| e.to_string());
        Ok(Patterns {
            import_from: re(r#"\bimport\s+(?:type\s+)?([^'";]*?)\s*from\s*["']([^"']+)["']"#)?,
            bare_import: re(r#"\bimport\s*\(\s*["']([^"']+)["']\s*\)|\brequire\s*\(\s*["']([^"']+)["']\s*\)|\bimport\s+["']([^"']+)["']"#)?,
            export_star: re(r#"\bexport\s+(?:type\s+)?\*\s*(?:as\s+([\w$]+)\s+)?from\s*["']([^"']+)["']"#)?,
            export_list: re(r#"\bexport\s+(?:type\s+)?\{([^}]*)\}(?:\s*from\s*["']([^"']+)["'])?"#)?,
            export_decl: re(r"\bexport\s+(?:declare\s+)?(?:default\s+)?(?:abstract\s+)?(?:async\s+)?(?:const|let|var|function\*?|class|interface|type|enum)\s+([\w$]+)")?,
        })
    }
}

/// Splits `{ a, type b, c as d }` into (original name, exported name) pairs.
fn brace_names(list: &str) -> Vec<(String, String)> {
    list.split(',').filter_map(|item| {
        let item = item.trim().trim_start_matches("type ").trim();
        if item.is_empty() { return None; }
        let mut names = item.split(" as ").map(str::trim);
        let original = names.next()?.to_owned();
        let exported = names.next().map(str::to_owned).unwrap_or_else(|| original.clone());
        Some((original, exported))
    }).collect()
}

/// The names an `import <clause> from` statement asks for.
fn imported_names(clause: &str) -> Names {
    let clause = clause.trim();
    if clause.contains('*') { return Names::All; }
    let (default_part, braces) = match clause.split_once('{') {
        Some((before, rest)) => (before, rest.trim_end_matches('}')),
        None => (clause, ""),
    };
    let mut names: Vec<String> = brace_names(braces).into_iter().map(|(original, _)| original).collect();
    if !default_part.trim().trim_end_matches(',').trim().is_empty() { names.push("default".to_owned()); }
    Names::List(names)
}

/// Collects (exported name, file or folder that provides it) for a module, following re-exports.
fn collect_exports(file: &Path, patterns: &Patterns, out: &mut Vec<(String, PathBuf)>, visited: &mut HashSet<PathBuf>, depth: usize) {
    if depth > MAX_REEXPORT_DEPTH || !visited.insert(file.to_path_buf()) { return; }
    let Ok(text) = fs::read_to_string(file) else { return; };
    let target_of = |spec: &str| spec.starts_with('.').then(|| normalize(&file.parent().unwrap_or(file).join(spec)));
    for cap in patterns.export_star.captures_iter(&text) {
        match (cap.get(1), target_of(&cap[2])) {
            // `export * as ns from './x'` is provided by x.
            (Some(namespace), target) => out.push((namespace.as_str().to_owned(), target.unwrap_or_else(|| file.to_path_buf()))),
            (None, Some(target)) => {
                if let Some(module) = resolve_module(&target) { collect_exports(&module, patterns, out, visited, depth + 1); }
            }
            (None, None) => {}
        }
    }
    for cap in patterns.export_list.captures_iter(&text) {
        // `export { a }` is provided by this file. `export { a } from './x'` is provided by whatever
        // x says provides a, so a chain of re-exports ends at the file that declares the name.
        let Some(target) = cap.get(2).and_then(|spec| target_of(spec.as_str())) else {
            out.extend(brace_names(&cap[1]).into_iter().map(|(_, exported)| (exported, file.to_path_buf())));
            continue;
        };
        let mut inner = vec![];
        if let Some(module) = resolve_module(&target) { collect_exports(&module, patterns, &mut inner, &mut visited.clone(), depth + 1); }
        for (original, exported) in brace_names(&cap[1]) {
            let provider = inner.iter().find(|(name, _)| name == &original).map(|(_, provider)| provider.clone()).unwrap_or_else(|| target.clone());
            out.push((exported, provider));
        }
    }
    out.extend(patterns.export_decl.captures_iter(&text).map(|cap| (cap[1].to_owned(), file.to_path_buf())));
    if text.contains("export default") { out.push(("default".to_owned(), file.to_path_buf())); }
}

/// For one slice: which segment, and which file in it, provides each name of its public API.
fn public_api(slice: &Slice, entry: Option<&Path>, patterns: &Patterns, root: &Path) -> HashMap<String, BTreeSet<(String, String)>> {
    let mut table: HashMap<String, BTreeSet<(String, String)>> = HashMap::new();
    let entry_file = match entry {
        Some(path) => resolve_module(path),
        None => index_file(&slice.dir),
    };
    let Some(index) = entry_file else { return table; };
    let mut exports = vec![];
    collect_exports(&index, patterns, &mut exports, &mut HashSet::new(), 0);
    for (name, provider) in exports {
        let segment = provider.strip_prefix(&slice.dir).ok()
            .and_then(|rel| rel.components().next())
            .map(|c| c.as_os_str().to_string_lossy().to_string())
            // A file directly in the slice is a segment named after the file.
            .map(|first| if is_source_file(Path::new(&first)) { Path::new(&first).file_stem().unwrap_or_default().to_string_lossy().to_string() } else { first })
            .filter(|first| slice.segments.contains(first));
        let provider_file = relative(root, &resolve_module(&provider).unwrap_or_else(|| provider.clone()));
        if let Some(segment) = segment { table.entry(name).or_default().insert((segment, provider_file)); }
    }
    table
}

pub fn scan(repo: &Path) -> Result<Value, String> {
    let root = find_source_root(repo)?;
    let scan = Scan::discover(&root);
    let paths = scan.node_paths();
    if paths.is_empty() { return Err(format!("FSD layer/slice/segment directory was not found beneath {}", root.display())); }

    let node_id = |path: &str| format!("observed-{}", path.replace('/', "--"));
    let mut per_layer: HashMap<&str, usize> = HashMap::new();
    let nodes: Vec<Value> = paths.iter().map(|path| {
        let layer = path.split('/').next().unwrap_or("");
        let column = per_layer.entry(layer).or_insert(0);
        let row = LAYERS.iter().position(|l| *l == layer).unwrap_or(0);
        let node = json!({"id":node_id(path),"path":path,"description":"실제 디렉터리에서 발견됨 · I/O 미분석","inputs":[],"outputs":[],"source":"filesystem","x":210 + *column * 300,"y":150 + row * 290});
        *column += 1;
        node
    }).collect();

    let patterns = Patterns::new()?;
    let mut api_cache: HashMap<(usize, Option<PathBuf>), HashMap<String, BTreeSet<(String, String)>>> = HashMap::new();
    // Per (importing node, imported node): every import as (importing file, name, providing file).
    let mut edge_order: Vec<(String, String)> = vec![];
    let mut edge_facts: HashMap<(String, String), BTreeSet<(String, String, String)>> = HashMap::new();
    let mut unresolved = 0usize;
    let mut unresolved_samples: Vec<Value> = vec![];
    let walker = WalkDir::new(&root).follow_links(false).max_depth(24).into_iter()
        .filter_entry(|e| e.depth() == 0 || !(e.file_type().is_dir() && is_ignored_dir(&e.file_name().to_string_lossy())));
    for entry in walker.filter_map(Result::ok) {
        let file = entry.path();
        if !entry.file_type().is_file() || !is_source_file(file) || is_test_file(file) { continue; }
        let Location::Node(source) = scan.locate(file) else { continue; };
        let Ok(text) = fs::read_to_string(file) else { continue; };

        let mut imports: Vec<(String, Names)> = vec![];
        imports.extend(patterns.import_from.captures_iter(&text).map(|cap| (cap[2].to_owned(), imported_names(&cap[1]))));
        imports.extend(patterns.export_star.captures_iter(&text).map(|cap| (cap[2].to_owned(), Names::All)));
        imports.extend(patterns.export_list.captures_iter(&text).filter_map(|cap| {
            let spec = cap.get(2)?.as_str().to_owned();
            Some((spec, Names::List(brace_names(&cap[1]).into_iter().map(|(original, _)| original).collect())))
        }));
        imports.extend(patterns.bare_import.captures_iter(&text).filter_map(|cap| {
            let spec = cap.get(1).or(cap.get(2)).or(cap.get(3))?;
            Some((spec.as_str().to_owned(), Names::All))
        }));

        for (spec, names) in imports {
            let Some(target_path) = scan.resolve_spec(file, &spec) else { continue; };
            // Each target comes with what is imported from it: (name, file that provides the name).
            let targets: Vec<(String, Vec<(String, String)>)> = match scan.locate(&target_path) {
                Location::Node(path) => {
                    let provider = relative(&root, &resolve_module(&target_path).unwrap_or_else(|| target_path.clone()));
                    let listed = match &names { Names::All => vec![ALL_NAMES.to_owned()], Names::List(list) => list.clone() };
                    vec![(path, listed.into_iter().map(|name| (name, provider.clone())).collect())]
                }
                Location::Api(index, entry) => {
                    let slice = &scan.slices[index];
                    let api = api_cache.entry((index, entry.clone())).or_insert_with(|| public_api(slice, entry.as_deref(), &patterns, &root));
                    let mut by_segment: BTreeMap<&String, Vec<(String, String)>> = BTreeMap::new();
                    match &names {
                        Names::All => for (segment, _) in api.values().flatten() {
                            by_segment.entry(segment).or_insert_with(|| vec![(ALL_NAMES.to_owned(), String::new())]);
                        },
                        Names::List(list) => for name in list {
                            match api.get(name) {
                                Some(found) => for (segment, provider) in found { by_segment.entry(segment).or_default().push((name.clone(), provider.clone())); },
                                None => {
                                    unresolved += 1;
                                    if unresolved_samples.len() < MAX_UNRESOLVED_SAMPLES {
                                        unresolved_samples.push(json!({"from":source,"import":spec,"name":name}));
                                    }
                                }
                            }
                        },
                    }
                    by_segment.into_iter().map(|(segment, imported)| (format!("{}/{}/{segment}", slice.layer, slice.rel.join("/")), imported)).collect()
                }
                Location::Outside => vec![],
            };
            let importing_file = relative(&root, file);
            for (target, imported) in targets {
                if target == source { continue; }
                let key = (source.clone(), target);
                if !edge_facts.contains_key(&key) { edge_order.push(key.clone()); }
                edge_facts.entry(key).or_default().extend(imported.into_iter().map(|(name, provider)| (importing_file.clone(), name, provider)));
            }
        }
    }

    let edges: Vec<Value> = edge_order.iter().enumerate().map(|(i, key)| {
        let imports: Vec<Value> = edge_facts[key].iter().map(|(file, name, provider)| json!({"file":file,"name":name,"provider":provider})).collect();
        json!({"id":format!("observed-edge-{}",i + 1),"from":node_id(&key.0),"to":node_id(&key.1),"type":"imports","source":"import-scan","imports":imports})
    }).collect();

    let source_root = root.strip_prefix(repo).unwrap_or(&root).to_string_lossy().replace('\\', "/");
    Ok(json!({"schemaVersion":"0.3","version":1,"nodes":nodes,"edges":edges,"requests":[],"repository":repo.display().to_string(),
        "analysis":{"kind":"static","sourceRoot":source_root,"slices":scan.slices.len(),"unresolvedImports":unresolved,"unresolvedSamples":unresolved_samples,
            "note":"Directories and resolvable imports are verified. A slice is the first folder below a layer that has an index file; imports of a slice's public API are traced through its re-exports. No AI-derived I/O contracts."}}))
}
