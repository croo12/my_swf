//! The directory hierarchy and imports are observations; no AI describes or reorganizes them.
use crate::{
    graph,
    plan::{Interface, Node},
};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    path::Path,
};

pub const LAYERS: [&str; 6] = ["app", "pages", "widgets", "features", "entities", "shared"];
const IGNORE: [&str; 9] = [
    "node_modules",
    "target",
    "dist",
    "build",
    "coverage",
    "__tests__",
    "__snapshots__",
    "__mocks__",
    "__fixtures__",
];
const EXTENSIONS: [&str; 10] = [
    "ts", "tsx", "js", "jsx", "mts", "mjs", "css", "scss", "json", "svg",
];

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Entry {
    pub name: String,
    /// Relative to the FSD source root, using forward slashes on every platform.
    pub path: String,
    pub kind: String,
    pub index: Option<String>,
    pub file_count: usize,
    pub children: Vec<Entry>,
}

impl Entry {
    pub fn find(&self, path: &str) -> Option<&Self> {
        if self.path == path {
            return Some(self);
        }
        self.children.iter().find_map(|child| child.find(path))
    }
    pub fn contains(&self, file: &str) -> bool {
        self.path.is_empty() || file == self.path || file.starts_with(&format!("{}/", self.path))
    }
    pub fn files(&self) -> Vec<String> {
        if self.kind == "file" {
            return vec![self.path.clone()];
        }
        self.children.iter().flat_map(Self::files).collect()
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Snapshot {
    pub repository: String,
    pub source_root: String,
    pub tree: Entry,
    /// Keep the existing scanner's facts, including per-file import names.
    pub scan: Value,
    /// Declaration categories from the same conservative rules as the existing module scanner.
    #[serde(default)]
    pub interface_kinds: BTreeMap<String, String>,
}

impl Snapshot {
    /// Crossing the selected group's boundary, rather than flattening all descendants.
    pub fn relations(&self, entry: &Entry, incoming: bool) -> Vec<(String, String)> {
        let mut found = BTreeSet::new();
        for record in self.scan["records"].as_array().into_iter().flatten() {
            let file = record["file"].as_str().unwrap_or_default();
            let provider = record["provider"].as_str().unwrap_or_default();
            let (inside, outside) = if incoming {
                (provider, file)
            } else {
                (file, provider)
            };
            if entry.contains(inside) && !entry.contains(outside) {
                found.insert((
                    outside.to_owned(),
                    record["name"].as_str().unwrap_or("*").to_owned(),
                ));
            }
        }
        found.into_iter().collect()
    }

    /// Seed a manually edited module with the observed files and import names of one group.
    pub fn module(&self, entry: &Entry) -> Node {
        let files = entry.files();
        let paths: Vec<&str> = self.scan["nodes"]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(|node| node["path"].as_str())
            .collect();
        let placement = paths
            .into_iter()
            .filter(|path| entry.path == *path || entry.path.starts_with(&format!("{path}/")))
            .max_by_key(|path| path.len())
            .map(str::to_owned)
            .unwrap_or_else(|| {
                // Whole slices/layers are imported as individual child groups by the UI.
                entry.path.clone()
            });
        let names: BTreeMap<String, String> = self.scan["records"]
            .as_array()
            .into_iter()
            .flatten()
            .filter(|record| entry.contains(record["provider"].as_str().unwrap_or_default()))
            .filter(|record| !entry.contains(record["file"].as_str().unwrap_or_default()))
            .filter_map(|record| {
                let name = record["name"].as_str()?;
                if name == "*" {
                    return None;
                }
                let provider = record["provider"].as_str()?;
                let kind = self
                    .interface_kinds
                    .get(&format!("{provider}#{name}"))
                    .cloned()
                    .unwrap_or_else(|| "function".into());
                Some((name.to_owned(), kind))
            })
            .collect();
        Node {
            name: entry.name.clone(),
            kind: "modify".into(),
            path: placement.clone(),
            files: files
                .iter()
                .map(|file| {
                    file.strip_prefix(&format!("{placement}/"))
                        .unwrap_or(file)
                        .to_owned()
                })
                .collect(),
            interfaces: names
                .into_iter()
                .map(|(name, kind)| Interface {
                    name,
                    kind,
                    ..Default::default()
                })
                .collect(),
            ..Default::default()
        }
    }
}

pub fn analyze(repo: &Path) -> Result<Snapshot, String> {
    let (root, slices) = graph::source_layout(repo)?;
    let canonical_repo = fs::canonicalize(repo).map_err(|e| e.to_string())?;
    let canonical_root = fs::canonicalize(&root).map_err(|e| e.to_string())?;
    if !canonical_root.starts_with(&canonical_repo) {
        return Err("FSD source root must be inside the selected repository".into());
    }
    let scan = graph::scan(repo)?;
    let interface_kinds = declaration_kinds(&root, &scan)?;
    let mut remaining = 50_000usize;
    let mut tree = read_entry(&root, "", &slices, 0, &mut remaining)?;
    // Root navigation shows the FSD layers only, in dependency order.
    tree.children
        .retain(|entry| LAYERS.contains(&entry.name.as_str()));
    tree.children
        .sort_by_key(|entry| LAYERS.iter().position(|layer| *layer == entry.name));
    tree.file_count = tree.children.iter().map(|entry| entry.file_count).sum();
    Ok(Snapshot {
        repository: repo.display().to_string(),
        source_root: root
            .strip_prefix(repo)
            .unwrap_or(&root)
            .to_string_lossy()
            .replace('\\', "/"),
        tree,
        scan,
        interface_kinds,
    })
}

fn declaration_kinds(root: &Path, scan: &Value) -> Result<BTreeMap<String, String>, String> {
    let pattern = regex::Regex::new(r"(?m)^\s*(?:export\s+)?(?:declare\s+)?(?:default\s+)?(?:abstract\s+)?(?:async\s+)?(const|let|var|function\*?|class|interface|type|enum)\s+([\w$]+)\s*(?::[^=\n]*)?=?\s*([^\n]{0,24})")
        .map_err(|e| e.to_string())?;
    let providers: BTreeSet<&str> = scan["records"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|record| record["provider"].as_str())
        .collect();
    let mut result = BTreeMap::new();
    for provider in providers {
        let Ok(text) = fs::read_to_string(root.join(provider)) else {
            continue;
        };
        for capture in pattern.captures_iter(&text) {
            let kind = if capture[3].trim().starts_with("createStore(")
                || capture[3].trim().starts_with("createStore<")
            {
                "store"
            } else {
                graph::interface_kind(
                    capture[1].trim_end_matches('*'),
                    &capture[2],
                    capture[3].trim(),
                    provider,
                )
            };
            result.insert(format!("{provider}#{}", &capture[2]), kind.to_owned());
        }
    }
    Ok(result)
}

fn read_entry(
    dir: &Path,
    relative: &str,
    slices: &[String],
    depth: usize,
    remaining: &mut usize,
) -> Result<Entry, String> {
    if depth > 32 || *remaining == 0 {
        return Err("Repository hierarchy exceeds 32 levels or 50,000 entries".into());
    }
    *remaining -= 1;
    let name = dir
        .file_name()
        .unwrap_or_default()
        .to_string_lossy()
        .to_string();
    let is_file = dir.is_file();
    let mut entry = Entry {
        name,
        path: relative.into(),
        kind: if is_file { "file" } else { "folder" }.into(),
        index: None,
        file_count: usize::from(is_file),
        children: vec![],
    };
    if is_file {
        return Ok(entry);
    }
    let children = fs::read_dir(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    for child in children {
        let child = child.map_err(|e| e.to_string())?;
        let file_type = child.file_type().map_err(|e| e.to_string())?;
        if file_type.is_symlink() {
            continue;
        }
        let child_name = child.file_name().to_string_lossy().to_string();
        if child_name.starts_with('.') || IGNORE.contains(&child_name.as_str()) {
            continue;
        }
        if depth == 0 && !LAYERS.contains(&child_name.as_str()) {
            continue;
        }
        if file_type.is_file()
            && !child
                .path()
                .extension()
                .and_then(|v| v.to_str())
                .is_some_and(|ext| EXTENSIONS.contains(&ext))
        {
            continue;
        }
        let child_path = if relative.is_empty() {
            child_name.clone()
        } else {
            format!("{relative}/{child_name}")
        };
        let nested = read_entry(&child.path(), &child_path, slices, depth + 1, remaining)?;
        if file_type.is_file()
            && matches!(
                child_name.as_str(),
                "index.ts" | "index.tsx" | "index.js" | "index.jsx" | "index.mts" | "index.mjs"
            )
        {
            entry.index = Some(child_path);
        }
        entry.file_count += nested.file_count;
        entry.children.push(nested);
    }
    entry
        .children
        .sort_by(|a, b| (a.kind == "file", &a.name).cmp(&(b.kind == "file", &b.name)));
    entry.kind = if relative.is_empty() {
        "root"
    } else if depth == 1 {
        "layer"
    } else if slices.iter().any(|slice| slice == relative) {
        "slice"
    } else if entry.index.is_some() {
        "group"
    } else {
        "folder"
    }
    .into();
    Ok(entry)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn keeps_declared_types_and_stores_when_seeding_a_manual_plan() {
        let temp = tempfile::tempdir().unwrap();
        fs::write(temp.path().join("contracts.ts"), "export interface Cart { id: string }\nexport const cartStore = createStore({});\nexport const add = (id: string) => id;").unwrap();
        let scan = serde_json::json!({"records":[{"provider":"contracts.ts"}]});
        let kinds = declaration_kinds(temp.path(), &scan).unwrap();
        assert_eq!(kinds["contracts.ts#Cart"], "type");
        assert_eq!(kinds["contracts.ts#cartStore"], "store");
        assert_eq!(kinds["contracts.ts#add"], "function");
    }

    #[test]
    fn retains_nested_index_groups_and_actual_dependency_boundaries() {
        let temp = tempfile::tempdir().unwrap();
        for (file, text) in [
            (
                "src/features/cart/index.ts",
                "export { add } from './model/actions';",
            ),
            (
                "src/features/cart/model/actions/index.ts",
                "export { add } from './add';",
            ),
            (
                "src/features/cart/model/actions/add.ts",
                "import { product } from '@/entities/product'; export const add = () => product;",
            ),
            (
                "src/entities/product/index.ts",
                "export { product } from './model/product';",
            ),
            (
                "src/entities/product/model/product.ts",
                "export const product = 1;",
            ),
            ("node_modules/ignored/index.ts", ""),
        ] {
            let path = temp.path().join(file);
            fs::create_dir_all(path.parent().unwrap()).unwrap();
            fs::write(path, text).unwrap();
        }
        let snapshot = analyze(temp.path()).unwrap();
        assert_eq!(snapshot.source_root, "src");
        assert_eq!(snapshot.tree.children.len(), 2);
        assert_eq!(snapshot.tree.find("features/cart").unwrap().kind, "slice");
        let group = snapshot.tree.find("features/cart/model/actions").unwrap();
        assert_eq!(group.kind, "group");
        assert_eq!(
            group.index.as_deref(),
            Some("features/cart/model/actions/index.ts")
        );
        assert_eq!(group.children.len(), 2);
        assert_eq!(
            snapshot.relations(group, false),
            vec![("entities/product/model/product.ts".into(), "product".into())]
        );
        let node = snapshot.module(group);
        assert_eq!(node.path, "features/cart/model");
        assert!(node.files.contains(&"actions/add.ts".into()));
    }
}
