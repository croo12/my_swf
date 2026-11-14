use factory_repository::projection::{Edge, Graph};
use std::collections::BTreeSet;

pub fn inside(path: &str, group: &str) -> bool {
    path == group || path.starts_with(&format!("{group}/"))
}

pub fn is_layer(path: &str) -> bool {
    factory_repository::LAYERS.contains(&path)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Direction {
    Incoming,
    Outgoing,
}

// For an expanded block, its descendants still represent the selected block's boundary.
// Internal edges and transitive dependencies do not belong to that boundary.
pub fn relation(edge: &Edge, selected: &str) -> Option<Direction> {
    if selected.is_empty() || is_layer(selected) {
        return None;
    }
    match (inside(&edge.from, selected), inside(&edge.to, selected)) {
        (true, false) => Some(Direction::Outgoing),
        (false, true) => Some(Direction::Incoming),
        _ => None,
    }
}

pub(crate) struct Focus {
    selected: String,
    related: BTreeSet<String>,
}
impl Focus {
    pub(crate) fn new(graph: &Graph, selected: &str) -> Self {
        let selected = if is_layer(selected) { "" } else { selected };
        let mut related = BTreeSet::new();
        let mut include = |path: &str| {
            let mut path = path;
            loop {
                related.insert(path.to_owned());
                let Some((parent, _)) = path.rsplit_once('/') else {
                    break;
                };
                path = parent;
            }
        };
        if !selected.is_empty() {
            include(selected);
            for edge in &graph.edges {
                if relation(edge, selected).is_some() {
                    include(&edge.from);
                    include(&edge.to);
                }
            }
        }
        Self {
            selected: selected.into(),
            related,
        }
    }
    pub(crate) fn dimmed(&self, path: &str) -> bool {
        !self.selected.is_empty() && !is_layer(path) && !self.related.contains(path)
    }
    pub(crate) fn rank(&self, path: &str) -> u8 {
        if self.selected.is_empty() || inside(&self.selected, path) {
            0
        } else if self.related.contains(path) {
            1
        } else {
            2
        }
    }
}
