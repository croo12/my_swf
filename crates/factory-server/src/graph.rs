//! FSD-aware, conservative repository scanner.
//! Filesystem layout and resolvable import edges are observed; I/O contracts are NOT guessed.
use std::{collections::{HashMap, HashSet}, fs, path::{Path, PathBuf}};
use regex::Regex;
use serde_json::{json, Value};
use walkdir::WalkDir;

const LAYERS: &[&str] = &["app", "pages", "widgets", "features", "entities", "shared"];

fn fsd_path(relative: &Path) -> Option<String> {
    let parts: Vec<String> = relative.components().map(|p| p.as_os_str().to_string_lossy().to_string()).collect();
    let layer = parts.first()?.as_str();
    if !LAYERS.contains(&layer) { return None; }
    if layer == "app" || layer == "shared" {
        if parts.len() < 2 { return None; }
        Some(format!("{}/{}", layer, parts[1]))
    } else {
        if parts.len() < 3 { return None; }
        Some(format!("{}/{}/{}", layer, parts[1], parts[2]))
    }
}

fn source_root(repo: &Path) -> PathBuf {
    if repo.join("src").is_dir() { repo.join("src") } else { repo.to_path_buf() }
}

fn normalize_import_path(from_file: &Path, spec: &str, root: &Path) -> Option<PathBuf> {
    let logical = if let Some(x) = spec.strip_prefix("@/") {
        root.join(x)
    } else if spec.starts_with('.') {
        from_file.parent()?.join(spec)
    } else {
        // Try aliases like "entities/book/model"; skip npm packages otherwise.
        if LAYERS.iter().any(|layer| spec.starts_with(&format!("{layer}/"))) { root.join(spec) }
        else { return None; }
    };
    // Lexical normalization only. Do not escape the selected repository's source root.
    let mut path = PathBuf::new();
    for c in logical.components() {
        match c {
            std::path::Component::ParentDir => { path.pop(); }
            std::path::Component::CurDir => {}
            other => path.push(other.as_os_str()),
        }
    }
    path.starts_with(root).then_some(path)
}

pub fn scan(repo: &Path) -> Result<Value, String> {
    let root = source_root(repo);
    let mut paths = Vec::<String>::new();
    let mut seen = HashSet::new();
    for layer in LAYERS {
        let base = root.join(layer);
        if !base.is_dir() { continue; }
        let Ok(dirs) = fs::read_dir(&base) else { continue; };
        for a in dirs.flatten() {
            if !a.path().is_dir() { continue; }
            if *layer == "app" || *layer == "shared" {
                let candidate = format!("{layer}/{}", a.file_name().to_string_lossy());
                if seen.insert(candidate.clone()) { paths.push(candidate); }
            } else if let Ok(subdirs) = fs::read_dir(a.path()) {
                for b in subdirs.flatten() {
                    if !b.path().is_dir() { continue; }
                    let candidate = format!("{layer}/{}/{}", a.file_name().to_string_lossy(), b.file_name().to_string_lossy());
                    if seen.insert(candidate.clone()) { paths.push(candidate); }
                }
            }
        }
    }
    if paths.is_empty() { return Err(format!("FSD layer/slice/segment directory was not found beneath {}", root.display())); }
    paths.sort_by_key(|s| (LAYERS.iter().position(|l| *l == s.split('/').next().unwrap_or("")).unwrap_or(99), s.clone()));
    let mut per_layer: HashMap<String, usize> = HashMap::new();
    let mut nodes = vec![];
    let mut ids = HashMap::new();
    for path in &paths {
        let layer = path.split('/').next().unwrap_or("");
        let index = per_layer.entry(layer.to_owned()).or_insert(0);
        let row = LAYERS.iter().position(|l| *l == layer).unwrap_or(0);
        let id = format!("observed-{}", path.replace('/', "--"));
        ids.insert(path.clone(), id.clone());
        nodes.push(json!({"id":id,"path":path,"description":"실제 디렉터리에서 발견됨 · I/O 미분석","inputs":[],"outputs":[],"source":"filesystem","x":210 + *index * 300,"y":150 + row * 290}));
        *index += 1;
    }
    let re = Regex::new(r#"(?:\bfrom\s*|\bimport\s*\(\s*|\brequire\s*\(\s*)[\"']([^\"']+)[\"']"#).map_err(|e| e.to_string())?;
    let mut edges = vec![];
    let mut edge_ids = HashSet::new();
    for entry in WalkDir::new(&root).follow_links(false).max_depth(16).into_iter().filter_map(Result::ok) {
        if !entry.file_type().is_file() { continue; }
        let path = entry.path();
        let Some(ext) = path.extension().and_then(|e| e.to_str()) else { continue; };
        if !["ts", "tsx", "js", "jsx", "mts", "mjs"].contains(&ext) { continue; }
        if path.components().any(|p| ["node_modules", "dist", ".git", "coverage"].iter().any(|part| p.as_os_str() == *part)) { continue; }
        let Ok(relative) = path.strip_prefix(&root) else { continue; };
        let Some(source_module) = fsd_path(relative) else { continue; };
        if !ids.contains_key(&source_module) { continue; }
        let Ok(text) = fs::read_to_string(path) else { continue; };
        for cap in re.captures_iter(&text) {
            let Some(target_path) = normalize_import_path(path, &cap[1], &root) else { continue; };
            let Ok(target_rel) = target_path.strip_prefix(&root) else { continue; };
            let Some(target_module) = fsd_path(target_rel) else { continue; };
            if target_module == source_module { continue; }
            let (Some(from), Some(to)) = (ids.get(&source_module), ids.get(&target_module)) else { continue; };
            if edge_ids.insert((from.clone(), to.clone())) {
                edges.push(json!({"id":format!("observed-edge-{}",edge_ids.len()),"from":from,"to":to,"type":"imports","source":"import-scan"}));
            }
        }
    }
    Ok(json!({"schemaVersion":"0.3","version":1,"nodes":nodes,"edges":edges,"requests":[],"repository":repo.display().to_string(),"analysis":{"kind":"static","note":"Directories and some import paths are verified. No AI-derived I/O contracts; tsconfig aliases/re-exports may need additional analysis."}}))
}
