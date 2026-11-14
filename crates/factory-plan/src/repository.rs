use crate::{Interface, Node};
use factory_repository::{Entry, Snapshot};
use std::collections::BTreeMap;

#[cfg(test)]
mod tests;

impl Node {
    /// Seed a manually edited module with the observed files and import names of one group.
    pub fn from_repository(snapshot: &Snapshot, entry: &Entry) -> Self {
        let files = entry.files();
        let paths: Vec<&str> = snapshot.scan["nodes"]
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
        let names: BTreeMap<String, String> = snapshot.scan["records"]
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
                let kind = snapshot
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
