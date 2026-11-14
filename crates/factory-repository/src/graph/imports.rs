use super::*;

impl Patterns {
    pub(super) fn new() -> Result<Patterns, String> {
        let re = |pattern: &str| Regex::new(pattern).map_err(|e| e.to_string());
        Ok(Patterns {
            import_from: re(r#"\bimport\s+(?:type\s+)?([^'";]*?)\s*from\s*["']([^"']+)["']"#)?,
            bare_import: re(
                r#"\bimport\s*\(\s*["']([^"']+)["']\s*\)|\brequire\s*\(\s*["']([^"']+)["']\s*\)|\bimport\s+["']([^"']+)["']"#,
            )?,
            export_star: re(
                r#"\bexport\s+(?:type\s+)?\*\s*(?:as\s+([\w$]+)\s+)?from\s*["']([^"']+)["']"#,
            )?,
            export_list: re(
                r#"\bexport\s+(?:type\s+)?\{([^}]*)\}(?:\s*from\s*["']([^"']+)["'])?"#,
            )?,
            export_decl: re(
                r"\bexport\s+(?:declare\s+)?(?:default\s+)?(?:abstract\s+)?(?:async\s+)?(?:const|let|var|function\*?|class|interface|type|enum)\s+([\w$]+)",
            )?,
        })
    }
}

/// Splits `{ a, type b, c as d }` into (original name, exported name) pairs.
pub(super) fn brace_names(list: &str) -> Vec<(String, String)> {
    list.split(',')
        .filter_map(|item| {
            let item = item.trim().trim_start_matches("type ").trim();
            if item.is_empty() {
                return None;
            }
            let mut names = item.split(" as ").map(str::trim);
            let original = names.next()?.to_owned();
            let exported = names
                .next()
                .map(str::to_owned)
                .unwrap_or_else(|| original.clone());
            Some((original, exported))
        })
        .collect()
}

/// The names an `import <clause> from` statement asks for.
pub(super) fn imported_names(clause: &str) -> Names {
    let clause = clause.trim();
    if clause.contains('*') {
        return Names::All;
    }
    let (default_part, braces) = match clause.split_once('{') {
        Some((before, rest)) => (before, rest.trim_end_matches('}')),
        None => (clause, ""),
    };
    let mut names: Vec<String> = brace_names(braces)
        .into_iter()
        .map(|(original, _)| original)
        .collect();
    if !default_part.trim().trim_end_matches(',').trim().is_empty() {
        names.push("default".to_owned());
    }
    Names::List(names)
}

/// Collects (exported name, file or folder that provides it) for a module, following re-exports.
pub(super) fn collect_exports(
    file: &Path,
    patterns: &Patterns,
    out: &mut Vec<(String, PathBuf)>,
    visited: &mut HashSet<PathBuf>,
    depth: usize,
) {
    if depth > MAX_REEXPORT_DEPTH || !visited.insert(file.to_path_buf()) {
        return;
    }
    let Ok(text) = fs::read_to_string(file) else {
        return;
    };
    let target_of = |spec: &str| {
        spec.starts_with('.')
            .then(|| normalize(&file.parent().unwrap_or(file).join(spec)))
    };
    for cap in patterns.export_star.captures_iter(&text) {
        match (cap.get(1), target_of(&cap[2])) {
            // `export * as ns from './x'` is provided by x.
            (Some(namespace), target) => out.push((
                namespace.as_str().to_owned(),
                target.unwrap_or_else(|| file.to_path_buf()),
            )),
            (None, Some(target)) => {
                if let Some(module) = resolve_module(&target) {
                    collect_exports(&module, patterns, out, visited, depth + 1);
                }
            }
            (None, None) => {}
        }
    }
    for cap in patterns.export_list.captures_iter(&text) {
        // `export { a }` is provided by this file. `export { a } from './x'` is provided by whatever
        // x says provides a, so a chain of re-exports ends at the file that declares the name.
        let Some(target) = cap.get(2).and_then(|spec| target_of(spec.as_str())) else {
            out.extend(
                brace_names(&cap[1])
                    .into_iter()
                    .map(|(_, exported)| (exported, file.to_path_buf())),
            );
            continue;
        };
        let mut inner = vec![];
        if let Some(module) = resolve_module(&target) {
            collect_exports(
                &module,
                patterns,
                &mut inner,
                &mut visited.clone(),
                depth + 1,
            );
        }
        for (original, exported) in brace_names(&cap[1]) {
            let provider = inner
                .iter()
                .find(|(name, _)| name == &original)
                .map(|(_, provider)| provider.clone())
                .unwrap_or_else(|| target.clone());
            out.push((exported, provider));
        }
    }
    out.extend(
        patterns
            .export_decl
            .captures_iter(&text)
            .map(|cap| (cap[1].to_owned(), file.to_path_buf())),
    );
    if text.contains("export default") {
        out.push(("default".to_owned(), file.to_path_buf()));
    }
}

/// For one slice: which segment, and which file in it, provides each name of its public API.
pub(super) fn public_api(
    slice: &Slice,
    entry: Option<&Path>,
    patterns: &Patterns,
    root: &Path,
) -> HashMap<String, BTreeSet<(String, String)>> {
    let mut table: HashMap<String, BTreeSet<(String, String)>> = HashMap::new();
    let entry_file = match entry {
        Some(path) => resolve_module(path),
        None => index_file(&slice.dir),
    };
    let Some(index) = entry_file else {
        return table;
    };
    let mut exports = vec![];
    collect_exports(&index, patterns, &mut exports, &mut HashSet::new(), 0);
    for (name, provider) in exports {
        let segment = provider
            .strip_prefix(&slice.dir)
            .ok()
            .and_then(|rel| rel.components().next())
            .map(|c| c.as_os_str().to_string_lossy().to_string())
            // A file directly in the slice is a segment named after the file.
            .map(|first| {
                if is_source_file(Path::new(&first)) {
                    Path::new(&first)
                        .file_stem()
                        .unwrap_or_default()
                        .to_string_lossy()
                        .to_string()
                } else {
                    first
                }
            })
            .filter(|first| slice.segments.contains(first));
        let provider_file = relative(
            root,
            &resolve_module(&provider).unwrap_or_else(|| provider.clone()),
        );
        if let Some(segment) = segment {
            table
                .entry(name)
                .or_default()
                .insert((segment, provider_file));
        }
    }
    table
}
