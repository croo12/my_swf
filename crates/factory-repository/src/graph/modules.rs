use super::*;

/// What kind of interface a declared name is, judged from its declaration and its name.
pub(crate) fn interface_kind(keyword: &str, name: &str, value: &str, file: &str) -> &'static str {
    let is_markup = file.ends_with(".tsx") || file.ends_with(".jsx");
    let mut chars = name.chars();
    let (first, second) = (chars.next().unwrap_or('_'), chars.nth(2).unwrap_or('_'));
    let is_hook = name.starts_with("use") && second.is_uppercase();
    let is_shouting =
        name.chars().any(char::is_alphabetic) && !name.chars().any(char::is_lowercase);
    match keyword {
        "interface" | "type" => "type",
        "enum" => "constant",
        _ if is_hook => "hook",
        _ if is_shouting => "constant",
        _ if first.is_uppercase() && is_markup => "component",
        "class" => "function",
        "const" | "let" | "var"
            if !(value.starts_with('(')
                || value.starts_with("async")
                || value.starts_with("function")
                || value.starts_with('<')) =>
        {
            "constant"
        }
        _ => "function",
    }
}

/// The modules of one slice as they exist on disk, in the shape of plan modules.
///
/// A module is one direct child of a segment: a file, or a folder with everything in it. Its
/// interfaces are the names other files import from it, and its dependencies are its imports from
/// the other modules of the slice. `incoming` and `outgoing` list the code outside the slice that
/// uses a module or that a module uses. Everything here is read from the repository.
pub fn slice_modules(repo: &Path, scan: &Value, key: &str) -> Result<Value, String> {
    let root = find_source_root(repo)?;
    let placements: Vec<&str> = scan
        .get("nodes")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|n| n.get("path").and_then(Value::as_str))
        .filter(|path| *path == key || path.starts_with(&format!("{key}/")))
        .collect();
    if placements.is_empty() {
        return Err(format!("{key} is not a slice of the analyzed repository."));
    }

    struct Unit {
        id: String,
        name: String,
        placement: String,
        files: Vec<String>,
    }
    let mut units: Vec<Unit> = vec![];
    for placement in &placements {
        let dir = root.join(placement);
        let mut add = |name: String, files: Vec<String>| {
            if !files.is_empty() {
                units.push(Unit {
                    id: format!("m-{}--{name}", placement.replace('/', "--")),
                    name,
                    placement: (*placement).to_owned(),
                    files,
                });
            }
        };
        if !dir.is_dir() {
            // A segment that is a single file.
            if let Some(file) = resolve_module(&dir) {
                add(
                    placement.rsplit('/').next().unwrap_or(placement).to_owned(),
                    vec![relative(&root, &file)],
                );
            }
            continue;
        }
        let mut children: Vec<PathBuf> = fs::read_dir(&dir)
            .into_iter()
            .flatten()
            .flatten()
            .map(|e| e.path())
            .collect();
        children.sort();
        for child in children {
            let name = child
                .file_name()
                .unwrap_or_default()
                .to_string_lossy()
                .to_string();
            if child.is_dir() {
                if is_ignored_dir(&name) || name == CROSS_API_DIR {
                    continue;
                }
                let mut files: Vec<String> = WalkDir::new(&child)
                    .into_iter()
                    .filter_map(Result::ok)
                    .filter(|e| {
                        e.file_type().is_file()
                            && is_source_file(e.path())
                            && !is_test_file(e.path())
                    })
                    .map(|e| relative(&root, e.path()))
                    .collect();
                files.sort();
                add(name, files);
            } else if is_source_file(&child) && !is_test_file(&child) {
                let stem = child
                    .file_stem()
                    .unwrap_or_default()
                    .to_string_lossy()
                    .to_string();
                // The index of a segment is its public API, not a module.
                if stem != ROOT_SEGMENT {
                    add(stem, vec![relative(&root, &child)]);
                }
            }
        }
    }
    let unit_of: HashMap<&str, usize> = units
        .iter()
        .enumerate()
        .flat_map(|(i, unit)| unit.files.iter().map(move |file| (file.as_str(), i)))
        .collect();

    let declaration = Regex::new(r"(?m)^\s*(?:export\s+)?(?:declare\s+)?(?:default\s+)?(?:abstract\s+)?(?:async\s+)?(const|let|var|function\*?|class|interface|type|enum)\s+([\w$]+)\s*(?::[^=\n]*)?=?\s*([^\n]{0,12})").map_err(|e| e.to_string())?;
    let mut kinds: HashMap<String, HashMap<String, &'static str>> = HashMap::new();
    let mut kind_of = |file: &str, name: &str| -> &'static str {
        let table = kinds.entry(file.to_owned()).or_insert_with(|| {
            let text = fs::read_to_string(root.join(file)).unwrap_or_default();
            let mut table = HashMap::new();
            for cap in declaration.captures_iter(&text) {
                table.entry(cap[2].to_owned()).or_insert_with(|| {
                    interface_kind(cap[1].trim_end_matches('*'), &cap[2], cap[3].trim(), file)
                });
            }
            table
        });
        table
            .get(name)
            .copied()
            .unwrap_or_else(|| interface_kind("", name, "(", file))
    };

    let mut interfaces: Vec<BTreeMap<String, &'static str>> =
        units.iter().map(|_| BTreeMap::new()).collect();
    let mut dependencies: BTreeMap<(usize, usize), BTreeSet<String>> = BTreeMap::new();
    let mut incoming: BTreeMap<(usize, String), BTreeSet<String>> = BTreeMap::new();
    let mut outgoing: BTreeMap<(usize, String), BTreeSet<String>> = BTreeMap::new();
    let text = |record: &Value, field: &str| {
        record
            .get(field)
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_owned()
    };
    for record in scan
        .get("records")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
    {
        let (from, file, to, provider, name) = (
            text(record, "from"),
            text(record, "file"),
            text(record, "to"),
            text(record, "provider"),
            text(record, "name"),
        );
        let (importer, owner) = (
            unit_of.get(file.as_str()).copied(),
            unit_of.get(provider.as_str()).copied(),
        );
        let named = name != ALL_NAMES;
        match (importer, owner) {
            (Some(a), Some(b)) if a == b => {}
            (importer, Some(b)) => {
                if named {
                    interfaces[b].insert(name.clone(), kind_of(&provider, &name));
                }
                let names = match importer {
                    Some(a) => dependencies.entry((a, b)).or_default(),
                    None => incoming.entry((b, from)).or_default(),
                };
                if named {
                    names.insert(name);
                }
            }
            // The module imports something outside the slice.
            (Some(a), None) if !placements.contains(&to.as_str()) => {
                let names = outgoing.entry((a, to)).or_default();
                if named {
                    names.insert(name);
                }
            }
            _ => {}
        }
    }

    let nodes: Vec<Value> = units.iter().enumerate().map(|(i, unit)| {
        let prefix = format!("{}/", unit.placement);
        let files: Vec<&str> = unit.files.iter().map(|file| file.strip_prefix(&prefix).unwrap_or_else(|| file.rsplit('/').next().unwrap_or(file))).collect();
        let listed: Vec<Value> = interfaces[i].iter().map(|(name, kind)| json!({"name":name,"kind":kind,"condition":"","result":"","inputs":[],"outputs":[]})).collect();
        json!({"id":unit.id,"name":unit.name,"kind":"existing","path":unit.placement,"description":"","interfaces":listed,"files":files})
    }).collect();
    let edges: Vec<Value> = dependencies.iter().enumerate().map(|(i, ((a, b), names))| {
        json!({"id":format!("{}--dep-{}",units[*a].id,i + 1),"from":units[*a].id,"to":units[*b].id,"type":"imports","uses":names})
    }).collect();
    let outside = |list: &BTreeMap<(usize, String), BTreeSet<String>>| -> Vec<Value> {
        list.iter().map(|((unit, placement), names)| json!({"module":units[*unit].id,"placement":placement,"names":names})).collect()
    };
    Ok(
        json!({"slice":key,"nodes":nodes,"edges":edges,"incoming":outside(&incoming),"outgoing":outside(&outgoing)}),
    )
}
