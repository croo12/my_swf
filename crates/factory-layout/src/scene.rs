use crate::{arrangement::HEADER, focus::relation};
use emath::{Pos2, Rect, Vec2};
use factory_core::repository_graph::{Edge, Graph, Node};
use std::collections::HashMap;

pub struct BoxItem {
    pub path: String,
    pub name: String,
    pub kind: String,
    pub index: bool,
    pub file_count: usize,
    pub rect: Rect,
    pub expanded: bool,
    pub expandable: bool,
    pub depth: usize,
    pub dimmed: bool,
}
impl BoxItem {
    pub fn header(&self) -> Rect {
        if self.expanded {
            Rect::from_min_size(self.rect.min, emath::vec2(self.rect.width(), HEADER))
        } else {
            self.rect
        }
    }
}

pub struct Route {
    pub edge: Edge,
    pub points: Vec<Pos2>,
}
impl Route {
    pub fn label_position(&self) -> Option<Pos2> {
        self.points
            .windows(2)
            .max_by(|a, b| {
                (a[1] - a[0])
                    .length_sq()
                    .total_cmp(&(b[1] - b[0]).length_sq())
            })
            .map(|segment| segment[0].lerp(segment[1], 0.5))
    }
}

pub struct Scene {
    pub boxes: Vec<BoxItem>,
    pub edges: Vec<Route>,
    pub size: Vec2,
}
struct RouteInput {
    ids: Vec<usize>,
    connections: Vec<(usize, usize)>,
    ports: Vec<(Pos2, Pos2, Pos2, Pos2)>,
    obstacles: Vec<Rect>,
}
impl Scene {
    pub fn from_elk(graph: Graph, layout: &crate::layout_engine::Layout) -> Self {
        fn flatten(
            nodes: &[Node],
            depth: usize,
            layout: &crate::layout_engine::Layout,
            boxes: &mut Vec<BoxItem>,
        ) {
            for node in nodes {
                boxes.push(BoxItem {
                    path: node.path.clone(),
                    name: node.name.clone(),
                    kind: node.kind.clone(),
                    index: node.index,
                    file_count: node.file_count,
                    rect: layout.nodes[&node.path],
                    expanded: !node.children.is_empty(),
                    expandable: node.expandable,
                    depth,
                    dimmed: false,
                });
                flatten(&node.children, depth + 1, layout, boxes);
            }
        }
        let mut boxes = vec![];
        flatten(&graph.nodes, 0, layout, &mut boxes);
        let edges = graph
            .edges
            .into_iter()
            .enumerate()
            .map(|(i, edge)| Route {
                edge,
                points: simplify(layout.edges[&crate::layout_engine::edge_id(i)].clone()),
            })
            .collect();
        Self {
            boxes,
            edges,
            size: layout.size,
        }
    }
    pub fn shows(&self, edge: &Edge, selected: &str) -> bool {
        relation(edge, selected).is_some()
    }
    pub fn new(graph: Graph) -> Self {
        let mut scene = Self::arrange(graph, "");
        let input = scene.route_input(None);
        if !input.connections.is_empty() {
            let mut router =
                crate::routing::Router::new(&input.obstacles, &input.ports, scene.size);
            for (id, (a, start, end, b)) in input.ids.into_iter().zip(input.ports) {
                let mut points = vec![a];
                points.extend(router.route(start, end));
                points.push(b);
                scene.edges[id].points = simplify(points);
            }
        }
        scene
    }
    pub fn focused(graph: Graph, selected: &str) -> Result<Self, String> {
        let mut scene = Self::arrange(graph, selected);
        let input = scene.route_input(Some(selected));
        let routes = crate::elk_routing::route(&input.obstacles, &input.connections, &input.ports)?;
        for (id, points) in input.ids.into_iter().zip(routes) {
            scene.edges[id].points = simplify(points);
        }
        // Include any outside routing tracks without changing the relative structure of the cards.
        let mut bounds = Rect::from_min_max(Pos2::ZERO, scene.size.to_pos2());
        for point in scene.edges.iter().flat_map(|route| &route.points) {
            bounds.extend_with(*point);
        }
        let shift = emath::vec2((-bounds.left()).max(0.0), (-bounds.top()).max(0.0));
        if shift != Vec2::ZERO {
            for item in &mut scene.boxes {
                item.rect = item.rect.translate(shift);
            }
            for point in scene.edges.iter_mut().flat_map(|route| &mut route.points) {
                *point += shift;
            }
        }
        scene.size = bounds.size() + Vec2::splat(16.0);
        Ok(scene)
    }
    fn route_input(&self, selected: Option<&str>) -> RouteInput {
        let by_path: HashMap<_, _> = self
            .boxes
            .iter()
            .enumerate()
            .map(|(i, item)| (item.path.as_str(), i))
            .collect();
        let rectangles: Vec<_> = self.boxes.iter().map(|item| item.rect).collect();
        let ids: Vec<_> = self
            .edges
            .iter()
            .enumerate()
            .filter(|(_, route)| {
                selected.is_none_or(|selected| relation(&route.edge, selected).is_some())
            })
            .map(|(id, _)| id)
            .collect();
        let connections: Vec<_> = ids
            .iter()
            .map(|&id| {
                let edge = &self.edges[id].edge;
                (by_path[edge.from.as_str()], by_path[edge.to.as_str()])
            })
            .collect();
        let ports = crate::routing::endpoints(&rectangles, &connections);
        let obstacles: Vec<_> = self
            .boxes
            .iter()
            .map(|item| {
                if item.expanded {
                    item.header().shrink2(emath::vec2(2.0, 0.0))
                } else {
                    item.rect
                }
            })
            .collect();
        RouteInput {
            ids,
            connections,
            ports,
            obstacles,
        }
    }
}

fn simplify(points: Vec<Pos2>) -> Vec<Pos2> {
    let mut result: Vec<Pos2> = vec![];
    for point in points {
        if result.last() == Some(&point) {
            continue;
        }
        while result.len() >= 2 {
            let a = result[result.len() - 2];
            let b = result[result.len() - 1];
            if !((a.x == b.x && b.x == point.x) || (a.y == b.y && b.y == point.y)) {
                break;
            }
            result.pop();
        }
        result.push(point);
    }
    result
}
