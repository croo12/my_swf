use super::*;

pub(super) fn is_ignored_dir(name: &str) -> bool {
    name.starts_with('.') || IGNORED_DIRS.contains(&name)
}
pub(super) fn is_direct_layer(layer: &str) -> bool {
    layer == "app" || layer == "shared"
}
pub(super) fn is_source_file(path: &Path) -> bool {
    path.extension()
        .and_then(|e| e.to_str())
        .is_some_and(|e| SOURCE_EXTS.contains(&e))
}
/// Tests and stories import widely for their own purposes; they are not part of the architecture.
pub(super) fn is_test_file(path: &Path) -> bool {
    let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
    [".test.", ".spec.", ".stories.", ".mock.", ".mocks."]
        .iter()
        .any(|marker| name.contains(marker))
}
pub(super) fn child_dirs(dir: &Path) -> Vec<(String, PathBuf)> {
    let mut dirs: Vec<(String, PathBuf)> = fs::read_dir(dir)
        .into_iter()
        .flatten()
        .flatten()
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
pub(super) fn segment_names(dir: &Path) -> Vec<String> {
    let mut names: BTreeSet<String> = child_dirs(dir)
        .into_iter()
        .map(|(name, _)| name)
        .filter(|name| name != CROSS_API_DIR)
        .collect();
    for entry in fs::read_dir(dir).into_iter().flatten().flatten() {
        let path = entry.path();
        if !path.is_file() || !is_source_file(&path) || is_test_file(&path) {
            continue;
        }
        let stem = path
            .file_stem()
            .unwrap_or_default()
            .to_string_lossy()
            .to_string();
        // "x.d.ts" has the stem "x.d"; declaration files are not segments.
        if stem != "index" && !stem.ends_with(".d") {
            names.insert(stem);
        }
    }
    names.into_iter().collect()
}
pub(super) fn index_file(dir: &Path) -> Option<PathBuf> {
    SOURCE_EXTS
        .iter()
        .map(|ext| dir.join(format!("index.{ext}")))
        .find(|p| p.is_file())
}
pub(super) fn has_direct_source(dir: &Path) -> bool {
    fs::read_dir(dir)
        .into_iter()
        .flatten()
        .flatten()
        .any(|e| e.path().is_file() && is_source_file(&e.path()))
}
pub(super) fn normalize(path: &Path) -> PathBuf {
    let mut out = PathBuf::new();
    for c in path.components() {
        match c {
            Component::ParentDir => {
                out.pop();
            }
            Component::CurDir => {}
            other => out.push(other.as_os_str()),
        }
    }
    out
}
/// A path as the plan writes it: relative to the source root, with forward slashes.
pub(super) fn relative(root: &Path, path: &Path) -> String {
    path.strip_prefix(root)
        .unwrap_or(path)
        .to_string_lossy()
        .replace('\\', "/")
}
/// Resolves an extension-less module path to a file: `x.ts`, `x.tsx`, ..., or `x/index.ts`.
pub(super) fn resolve_module(path: &Path) -> Option<PathBuf> {
    if path.is_file() {
        return Some(path.to_path_buf());
    }
    // Declaration files can only be reached through an import, so they are tried here but never scanned.
    for ext in SOURCE_EXTS.iter().copied().chain(["d.ts"]) {
        let mut with_ext = path.as_os_str().to_owned();
        with_ext.push(format!(".{ext}"));
        let candidate = PathBuf::from(with_ext);
        if candidate.is_file() {
            return Some(candidate);
        }
    }
    if path.is_dir() {
        index_file(path)
    } else {
        None
    }
}

/// Finds the directory that holds the FSD layer folders. `FACTORY_SOURCE_ROOT` (relative to the
/// repository, or absolute) overrides the search.
pub(super) fn find_source_root(repo: &Path) -> Result<PathBuf, String> {
    if let Ok(configured) = env::var("FACTORY_SOURCE_ROOT") {
        let root = repo.join(configured);
        return if root.is_dir() {
            Ok(root)
        } else {
            Err(format!(
                "FACTORY_SOURCE_ROOT is not a directory: {}",
                root.display()
            ))
        };
    }
    let layer_count = |dir: &Path| {
        LAYERS
            .iter()
            .filter(|layer| dir.join(layer).is_dir())
            .count()
    };
    let mut best: Option<(usize, usize, PathBuf)> = None; // (layers found, depth, path)
    let walker = WalkDir::new(repo)
        .follow_links(false)
        .max_depth(MAX_ROOT_DEPTH)
        .into_iter()
        .filter_entry(|e| e.depth() == 0 || !is_ignored_dir(&e.file_name().to_string_lossy()));
    for entry in walker
        .filter_map(Result::ok)
        .filter(|e| e.file_type().is_dir())
    {
        let score = layer_count(entry.path());
        // More layers wins; among equals the shallowest directory wins.
        if score >= 2
            && best
                .as_ref()
                .is_none_or(|(s, d, _)| score > *s || (score == *s && entry.depth() < *d))
        {
            best = Some((score, entry.depth(), entry.path().to_path_buf()));
        }
    }
    best.map(|(_, _, path)| path).ok_or_else(|| format!(
        "No directory with FSD layer folders (app/pages/widgets/features/entities/shared) was found within {} levels of {}. Set FACTORY_SOURCE_ROOT to point at it.",
        MAX_ROOT_DEPTH, repo.display()
    ))
}
