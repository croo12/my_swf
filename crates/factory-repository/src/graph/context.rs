use super::*;

impl Scan {
    pub(super) fn discover(root: &Path) -> Scan {
        let mut scan = Scan {
            root: root.to_path_buf(),
            slices: vec![],
            slice_index: HashMap::new(),
            direct_segments: HashMap::new(),
        };
        for &layer in LAYERS {
            let layer_dir = root.join(layer);
            if !layer_dir.is_dir() {
                continue;
            }
            if is_direct_layer(layer) {
                scan.direct_segments
                    .insert(layer, segment_names(&layer_dir));
            } else {
                for (name, dir) in child_dirs(&layer_dir) {
                    scan.collect_slices(layer, vec![name], &dir);
                }
            }
        }
        for (i, slice) in scan.slices.iter().enumerate() {
            scan.slice_index
                .insert((slice.layer.to_owned(), slice.rel.join("/")), i);
        }
        scan
    }

    /// Walks down from a layer: a folder with an index file (or with files/standard segments of its
    /// own) is a slice; anything above it is a grouping folder.
    pub(super) fn collect_slices(&mut self, layer: &'static str, rel: Vec<String>, dir: &Path) {
        let children = child_dirs(dir);
        let is_slice = index_file(dir).is_some()
            || has_direct_source(dir)
            || children
                .iter()
                .any(|(name, _)| STANDARD_SEGMENTS.contains(&name.as_str()));
        if is_slice {
            let mut segments = segment_names(dir);
            if segments.is_empty() {
                segments.push(ROOT_SEGMENT.to_owned());
            }
            self.slices.push(Slice {
                layer,
                rel,
                dir: dir.to_path_buf(),
                segments,
            });
        } else if rel.len() < MAX_GROUP_DEPTH {
            for (name, child) in children {
                let mut child_rel = rel.clone();
                child_rel.push(name);
                self.collect_slices(layer, child_rel, &child);
            }
        }
    }

    pub(super) fn node_paths(&self) -> Vec<String> {
        let mut paths = vec![];
        for &layer in LAYERS {
            if let Some(segments) = self.direct_segments.get(layer) {
                paths.extend(segments.iter().map(|segment| format!("{layer}/{segment}")));
            }
            for slice in self.slices.iter().filter(|s| s.layer == layer) {
                paths.extend(
                    slice
                        .segments
                        .iter()
                        .map(|segment| format!("{layer}/{}/{segment}", slice.rel.join("/"))),
                );
            }
        }
        paths
    }

    /// Maps a file or module path under the source root to the node (or slice) it belongs to.
    pub(super) fn locate(&self, path: &Path) -> Location {
        let Ok(relative) = path.strip_prefix(&self.root) else {
            return Location::Outside;
        };
        let mut parts: Vec<String> = relative
            .components()
            .map(|c| c.as_os_str().to_string_lossy().to_string())
            .collect();
        if let Some(last) = parts.last_mut()
            && is_source_file(Path::new(last.as_str()))
        {
            *last = Path::new(last.as_str())
                .file_stem()
                .unwrap_or_default()
                .to_string_lossy()
                .to_string();
        }
        let Some(layer) = parts.first().map(String::as_str) else {
            return Location::Outside;
        };
        if let Some(segments) = self.direct_segments.get(layer) {
            return match parts.get(1) {
                Some(segment) if segments.contains(segment) => {
                    Location::Node(format!("{layer}/{segment}"))
                }
                _ => Location::Outside,
            };
        }
        for end in 2..=parts.len() {
            let Some(&index) = self
                .slice_index
                .get(&(layer.to_owned(), parts[1..end].join("/")))
            else {
                continue;
            };
            let slice = &self.slices[index];
            let node = |segment: &str| {
                Location::Node(format!("{layer}/{}/{segment}", slice.rel.join("/")))
            };
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
    pub(super) fn resolve_spec(&self, from_file: &Path, spec: &str) -> Option<PathBuf> {
        let logical = if spec.starts_with('.') {
            from_file.parent()?.join(spec)
        } else if let Some(rest) = ["@/", "~/", "src/"]
            .iter()
            .find_map(|prefix| spec.strip_prefix(prefix))
        {
            self.root.join(rest)
        } else {
            // Layer aliases: "@features/x", "features/x".
            let bare = spec.strip_prefix('@').unwrap_or(spec);
            if !LAYERS.contains(&bare.split('/').next()?) {
                return None;
            }
            self.root.join(bare)
        };
        let path = normalize(&logical);
        path.starts_with(&self.root).then_some(path)
    }
}
