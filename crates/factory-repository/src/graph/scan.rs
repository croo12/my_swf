use super::*;

pub fn scan(repo: &Path) -> Result<Value, String> {
    let root = find_source_root(repo)?;
    let scan = Scan::discover(&root);
    let paths = scan.node_paths();
    if paths.is_empty() {
        return Err(format!(
            "FSD layer/slice/segment directory was not found beneath {}",
            root.display()
        ));
    }

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
    let mut api_cache: HashMap<(usize, Option<PathBuf>), PublicApi> = HashMap::new();
    // Per (importing node, imported node): every import as (importing file, name, providing file).
    let mut edge_order: Vec<(String, String)> = vec![];
    let mut edge_facts: HashMap<(String, String), ImportFacts> = HashMap::new();
    // Imports between files of one node. They make no edge, but they connect the modules inside it.
    let mut internal: BTreeMap<String, BTreeSet<(String, String, String)>> = BTreeMap::new();
    let mut unresolved = 0usize;
    let mut unresolved_samples: Vec<Value> = vec![];
    // The repository browser needs the actual module boundary, including index re-exports.
    // Keep these direct file references separate from the plan scanner's resolved providers.
    let mut file_references = BTreeSet::new();
    let walker = WalkDir::new(&root)
        .follow_links(false)
        .max_depth(24)
        .into_iter()
        .filter_entry(|e| {
            e.depth() == 0
                || !(e.file_type().is_dir() && is_ignored_dir(&e.file_name().to_string_lossy()))
        });
    for entry in walker.filter_map(Result::ok) {
        let file = entry.path();
        if !entry.file_type().is_file() || !is_source_file(file) || is_test_file(file) {
            continue;
        }
        let source = scan.locate(file);
        if !LAYERS.contains(&relative(&root, file).split('/').next().unwrap_or("")) {
            continue;
        }
        let Ok(text) = fs::read_to_string(file) else {
            continue;
        };

        let mut imports: Vec<(String, Names)> = vec![];
        imports.extend(
            patterns
                .import_from
                .captures_iter(&text)
                .map(|cap| (cap[2].to_owned(), imported_names(&cap[1]))),
        );
        imports.extend(
            patterns
                .export_star
                .captures_iter(&text)
                .map(|cap| (cap[2].to_owned(), Names::All)),
        );
        imports.extend(patterns.export_list.captures_iter(&text).filter_map(|cap| {
            let spec = cap.get(2)?.as_str().to_owned();
            Some((
                spec,
                Names::List(
                    brace_names(&cap[1])
                        .into_iter()
                        .map(|(original, _)| original)
                        .collect(),
                ),
            ))
        }));
        imports.extend(patterns.bare_import.captures_iter(&text).filter_map(|cap| {
            let spec = cap.get(1).or(cap.get(2)).or(cap.get(3))?;
            Some((spec.as_str().to_owned(), Names::All))
        }));

        for (spec, names) in imports {
            let Some(target_path) = scan.resolve_spec(file, &spec) else {
                continue;
            };
            if let Some(provider) = resolve_module(&target_path) {
                let imported = match &names {
                    Names::All => vec![ALL_NAMES.to_owned()],
                    Names::List(list) => list.clone(),
                };
                for name in imported {
                    file_references.insert((
                        relative(&root, file),
                        relative(&root, &provider),
                        name,
                    ));
                }
            }
            let Location::Node(source) = &source else {
                continue;
            };
            // Each target comes with what is imported from it: (name, file that provides the name).
            let targets: Vec<(String, Vec<(String, String)>)> = match scan.locate(&target_path) {
                Location::Node(path) => {
                    let provider = relative(
                        &root,
                        &resolve_module(&target_path).unwrap_or_else(|| target_path.clone()),
                    );
                    let listed = match &names {
                        Names::All => vec![ALL_NAMES.to_owned()],
                        Names::List(list) => list.clone(),
                    };
                    vec![(
                        path,
                        listed
                            .into_iter()
                            .map(|name| (name, provider.clone()))
                            .collect(),
                    )]
                }
                Location::Api(index, entry) => {
                    let slice = &scan.slices[index];
                    let api = api_cache
                        .entry((index, entry.clone()))
                        .or_insert_with(|| public_api(slice, entry.as_deref(), &patterns, &root));
                    let mut by_segment: BTreeMap<&String, Vec<(String, String)>> = BTreeMap::new();
                    match &names {
                        Names::All => {
                            for (segment, _) in api.values().flatten() {
                                by_segment
                                    .entry(segment)
                                    .or_insert_with(|| vec![(ALL_NAMES.to_owned(), String::new())]);
                            }
                        }
                        Names::List(list) => {
                            for name in list {
                                match api.get(name) {
                                    Some(found) => {
                                        for (segment, provider) in found {
                                            by_segment
                                                .entry(segment)
                                                .or_default()
                                                .push((name.clone(), provider.clone()));
                                        }
                                    }
                                    None => {
                                        unresolved += 1;
                                        if unresolved_samples.len() < MAX_UNRESOLVED_SAMPLES {
                                            unresolved_samples.push(
                                                json!({"from":source,"import":spec,"name":name}),
                                            );
                                        }
                                    }
                                }
                            }
                        }
                    }
                    by_segment
                        .into_iter()
                        .map(|(segment, imported)| {
                            (
                                format!("{}/{}/{segment}", slice.layer, slice.rel.join("/")),
                                imported,
                            )
                        })
                        .collect()
                }
                Location::Outside => vec![],
            };
            let importing_file = relative(&root, file);
            for (target, imported) in targets {
                let facts = imported
                    .into_iter()
                    .map(|(name, provider)| (importing_file.clone(), name, provider));
                if target == *source {
                    internal.entry(target).or_default().extend(facts);
                    continue;
                }
                let key = (source.clone(), target);
                if !edge_facts.contains_key(&key) {
                    edge_order.push(key.clone());
                }
                edge_facts.entry(key).or_default().extend(facts);
            }
        }
    }

    let edges: Vec<Value> = edge_order.iter().enumerate().map(|(i, key)| {
        json!({"id":format!("observed-edge-{}",i + 1),"from":node_id(&key.0),"to":node_id(&key.1),"type":"imports","source":"import-scan"})
    }).collect();
    // Every import: the placement and file that imports, the placement and file that provides, the name.
    let record = |from: &str, to: &str, (file, name, provider): &(String, String, String)| json!({"from":from,"file":file,"to":to,"provider":provider,"name":name});
    let records: Vec<Value> = edge_order
        .iter()
        .flat_map(|key| {
            edge_facts[key]
                .iter()
                .map(move |fact| (key.0.as_str(), key.1.as_str(), fact))
        })
        .chain(internal.iter().flat_map(|(path, facts)| {
            facts
                .iter()
                .map(move |fact| (path.as_str(), path.as_str(), fact))
        }))
        .map(|(from, to, fact)| record(from, to, fact))
        .collect();

    let source_root = root
        .strip_prefix(repo)
        .unwrap_or(&root)
        .to_string_lossy()
        .replace('\\', "/");
    let file_references: Vec<Value> = file_references
        .into_iter()
        .map(|(file, provider, name)| json!({"file":file,"provider":provider,"name":name}))
        .collect();
    Ok(
        json!({"schemaVersion":"0.3","version":1,"nodes":nodes,"edges":edges,"records":records,"fileReferences":file_references,"requests":[],"repository":repo.display().to_string(),
        "analysis":{"kind":"static","sourceRoot":source_root,"slices":scan.slices.len(),"unresolvedImports":unresolved,"unresolvedSamples":unresolved_samples,
            "note":"Directories and resolvable imports are verified. A slice is the first folder below a layer that has an index file; imports of a slice's public API are traced through its re-exports. No AI-derived I/O contracts."}}),
    )
}
