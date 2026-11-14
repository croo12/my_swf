//! Project actual file imports onto the boundaries currently visible in a compound graph.
use crate::repository::{Entry, Snapshot};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Clone, Debug)]
pub struct Node {
    pub path: String,
    pub name: String,
    pub kind: String,
    pub index: bool,
    pub file_count: usize,
    pub expandable: bool,
    pub children: Vec<Node>,
}

#[derive(Clone, Debug)]
pub struct Edge {
    pub from: String,
    pub to: String,
    pub file_pairs: usize,
    pub names: Vec<String>,
}

#[derive(Clone, Debug, Default)]
pub struct Graph {
    pub nodes: Vec<Node>,
    pub edges: Vec<Edge>,
}

impl Graph {
    pub fn node_count(&self) -> usize {
        fn count(node: &Node) -> usize {
            1 + node.children.iter().map(count).sum::<usize>()
        }
        self.nodes.iter().map(count).sum()
    }
}

/// Every expanded group stays in the graph as a container. Its siblings and other layers stay
/// visible. Imports terminate at the deepest visible child, or at a collapsed group's boundary.
pub fn project(snapshot: &Snapshot, expanded: &BTreeSet<String>) -> Graph {
    let mut owners = BTreeMap::new();
    fn visit(
        entry: &Entry,
        expanded: &BTreeSet<String>,
        owners: &mut BTreeMap<String, String>,
    ) -> Node {
        let expandable = !entry.children.is_empty();
        let children = if expandable && expanded.contains(&entry.path) {
            entry
                .children
                .iter()
                .map(|child| visit(child, expanded, owners))
                .collect()
        } else {
            for file in entry.files() {
                owners.insert(file, entry.path.clone());
            }
            vec![]
        };
        Node {
            path: entry.path.clone(),
            name: entry.name.clone(),
            kind: entry.kind.clone(),
            index: entry.index.is_some(),
            file_count: entry.file_count,
            expandable,
            children,
        }
    }
    let nodes = snapshot
        .tree
        .children
        .iter()
        .map(|entry| visit(entry, expanded, &mut owners))
        .collect();
    #[derive(Default)]
    struct Facts {
        pairs: BTreeSet<(String, String)>,
        names: BTreeSet<String>,
    }
    let mut connections: BTreeMap<(String, String), Facts> = BTreeMap::new();
    for record in snapshot.references() {
        let (Some(file), Some(provider)) = (record["file"].as_str(), record["provider"].as_str())
        else {
            continue;
        };
        let (Some(from), Some(to)) = (owners.get(file), owners.get(provider)) else {
            continue;
        };
        if from == to {
            continue;
        }
        let facts = connections.entry((from.clone(), to.clone())).or_default();
        facts.pairs.insert((file.into(), provider.into()));
        if let Some(name) = record["name"].as_str().filter(|name| *name != "*") {
            facts.names.insert(name.into());
        }
    }
    let edges = connections
        .into_iter()
        .map(|((from, to), facts)| Edge {
            from,
            to,
            file_pairs: facts.pairs.len(),
            names: facts.names.into_iter().collect(),
        })
        .collect();
    Graph { nodes, edges }
}

/// Reveal a search or relation target without collapsing unrelated parts of the graph.
pub fn reveal(tree: &Entry, path: &str, expanded: &mut BTreeSet<String>) {
    let mut parent = path;
    while let Some((ancestor, _)) = parent.rsplit_once('/') {
        if tree
            .find(ancestor)
            .is_some_and(|entry| !entry.children.is_empty())
        {
            expanded.insert(ancestor.into());
        }
        parent = ancestor;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    fn entry(path: &str, kind: &str, children: Vec<Entry>) -> Entry {
        Entry {
            name: path.rsplit('/').next().unwrap_or("root").into(),
            path: path.into(),
            kind: kind.into(),
            index: (kind == "group").then(|| format!("{path}/index.ts")),
            file_count: if children.is_empty() {
                1
            } else {
                children.iter().map(|child| child.file_count).sum()
            },
            children,
        }
    }
    fn fixture() -> Snapshot {
        let nested = entry(
            "features/cart/model/actions",
            "group",
            vec![
                entry("features/cart/model/actions/index.ts", "file", vec![]),
                entry("features/cart/model/actions/add.ts", "file", vec![]),
            ],
        );
        let cart = entry(
            "features/cart",
            "slice",
            vec![entry("features/cart/model", "folder", vec![nested])],
        );
        let checkout = entry(
            "features/checkout",
            "slice",
            vec![entry("features/checkout/ui.ts", "file", vec![])],
        );
        let shared = entry(
            "shared",
            "layer",
            vec![entry(
                "shared/api",
                "group",
                vec![entry("shared/api/client.ts", "file", vec![])],
            )],
        );
        Snapshot {
            repository: String::new(),
            source_root: "src".into(),
            tree: entry(
                "",
                "root",
                vec![entry("features", "layer", vec![cart, checkout]), shared],
            ),
            scan: json!({"records":[
                {"file":"features/cart/model/actions/add.ts","provider":"shared/api/client.ts","name":"request"},
                {"file":"features/cart/model/actions/add.ts","provider":"shared/api/client.ts","name":"Client"},
                {"file":"features/checkout/ui.ts","provider":"features/cart/model/actions/add.ts","name":"add"},
                {"file":"features/cart/model/actions/index.ts","provider":"features/cart/model/actions/add.ts","name":"add"}
            ]}),
            interface_kinds: BTreeMap::new(),
        }
    }
    #[test]
    fn collapsed_layers_aggregate_imports_without_counting_each_symbol_as_a_file_pair() {
        let graph = project(&fixture(), &BTreeSet::new());
        assert_eq!(graph.nodes.len(), 2);
        assert_eq!(graph.edges.len(), 1);
        assert_eq!(
            (&*graph.edges[0].from, &*graph.edges[0].to),
            ("features", "shared")
        );
        assert_eq!(graph.edges[0].file_pairs, 1);
        assert_eq!(graph.edges[0].names, ["Client", "request"]);
    }
    #[test]
    fn nested_expansion_retains_containers_siblings_and_cross_boundary_imports() {
        let snapshot = fixture();
        let mut expanded = BTreeSet::new();
        reveal(
            &snapshot.tree,
            "features/cart/model/actions/add.ts",
            &mut expanded,
        );
        let graph = project(&snapshot, &expanded);
        assert_eq!(graph.nodes.len(), 2); // shared remains visible
        assert_eq!(graph.nodes[0].children.len(), 2); // checkout remains visible
        let group = &graph.nodes[0].children[0].children[0].children[0];
        assert_eq!(group.path, "features/cart/model/actions");
        assert_eq!(group.children.len(), 2);
        assert!(
            graph
                .edges
                .iter()
                .any(|edge| edge.from == "features/checkout"
                    && edge.to == "features/cart/model/actions/add.ts")
        );
        assert!(
            graph.edges.iter().any(
                |edge| edge.from == "features/cart/model/actions/add.ts" && edge.to == "shared"
            )
        );
        assert!(
            graph
                .edges
                .iter()
                .any(|edge| edge.from.ends_with("index.ts") && edge.to.ends_with("add.ts"))
        );
    }
    #[test]
    fn collapsing_a_group_retargets_external_edges_and_hides_only_internal_edges() {
        let snapshot = fixture();
        let mut expanded = BTreeSet::new();
        reveal(
            &snapshot.tree,
            "features/cart/model/actions/add.ts",
            &mut expanded,
        );
        expanded.remove("features/cart/model/actions");
        let graph = project(&snapshot, &expanded);
        assert_eq!(graph.edges.len(), 2);
        assert!(graph.edges.iter().any(
            |edge| edge.from == "features/checkout" && edge.to == "features/cart/model/actions"
        ));
        assert!(
            graph
                .edges
                .iter()
                .any(|edge| edge.from == "features/cart/model/actions" && edge.to == "shared")
        );
        assert!(graph.edges.iter().all(|edge| edge.from != edge.to));
    }
}
