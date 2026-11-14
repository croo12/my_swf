use crate::{
    focus::Focus,
    scene::{BoxItem, Route, Scene},
};
use emath::{Pos2, Rect, Vec2};
use factory_core::repository_graph::{Graph, Node};

pub(crate) const HEADER: f32 = 58.0;
const PAD: f32 = 36.0;
const GAP: f32 = 48.0;
const LEAF: Vec2 = Vec2::new(234.0, 108.0);
pub(crate) const FOCUS_GAP: f32 = 180.0;

struct Branch<'a> {
    node: &'a Node,
    size: Vec2,
    children: Vec<(Pos2, Branch<'a>)>,
    active_end: f32,
}

fn measure<'a>(node: &'a Node, focus: &Focus, dock: Option<f32>) -> Branch<'a> {
    if node.children.is_empty() {
        let size = if node.kind == "layer" {
            emath::vec2(1050.0, 82.0)
        } else {
            LEAF
        };
        return Branch {
            node,
            size,
            children: vec![],
            active_end: if focus.dimmed(&node.path) {
                0.0
            } else {
                size.x
            },
        };
    }
    let mut ordered: Vec<_> = node.children.iter().collect();
    ordered.sort_by_key(|child| focus.rank(&child.path));
    let mut children = vec![];
    let (mut x, y, mut height, mut active_end) = (PAD, HEADER + PAD, 0.0f32, PAD);
    let mut docked = false;
    for child in ordered {
        let branch = measure(child, focus, None);
        if focus.dimmed(&child.path) && !docked {
            x += FOCUS_GAP;
            if let Some(dock) = dock {
                x = x.max(dock);
            }
            docked = true;
        }
        height = height.max(branch.size.y);
        let child_width = branch.size.x;
        if !focus.dimmed(&child.path) {
            active_end = x + child_width;
        }
        children.push((emath::pos2(x, y), branch));
        x += child_width + GAP;
    }
    Branch {
        node,
        size: emath::vec2(x - GAP + PAD, y + height + PAD),
        children,
        active_end,
    }
}

impl Scene {
    pub(crate) fn arrange(graph: Graph, selected: &str) -> Self {
        fn flatten(
            branch: &Branch<'_>,
            origin: Pos2,
            depth: usize,
            focus: &Focus,
            boxes: &mut Vec<BoxItem>,
        ) {
            boxes.push(BoxItem {
                path: branch.node.path.clone(),
                name: branch.node.name.clone(),
                kind: branch.node.kind.clone(),
                index: branch.node.index,
                file_count: branch.node.file_count,
                rect: Rect::from_min_size(origin, branch.size),
                expanded: !branch.children.is_empty(),
                expandable: branch.node.expandable,
                depth,
                dimmed: focus.dimmed(&branch.node.path),
            });
            for (offset, child) in &branch.children {
                flatten(child, origin + offset.to_vec2(), depth + 1, focus, boxes);
            }
        }
        let mut boxes = vec![];
        let focus = Focus::new(&graph, selected);
        let dock = graph
            .nodes
            .iter()
            .map(|node| measure(node, &focus, None).active_end)
            .fold(0.0f32, f32::max)
            + FOCUS_GAP;
        let (mut y, mut width) = (28.0, 1050.0f32);
        for node in &graph.nodes {
            let branch = measure(node, &focus, Some(dock));
            flatten(&branch, emath::pos2(50.0, y), 0, &focus, &mut boxes);
            width = width.max(branch.size.x);
            y += branch.size.y + 90.0;
        }
        let size = emath::vec2(width + 100.0, y + 32.0);
        Self {
            boxes,
            size,
            edges: graph
                .edges
                .into_iter()
                .map(|edge| Route {
                    edge,
                    points: vec![],
                })
                .collect(),
        }
    }
}
