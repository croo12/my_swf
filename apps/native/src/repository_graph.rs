use crate::canvas::{self, ACCENT, BACKGROUND, BORDER, CARD, MUTED, TEXT};
use eframe::egui::{self, Align2, Color32, FontId, Pos2, Rect, Sense, Stroke, StrokeKind, Vec2};
use factory_core::{
    repository::{Entry, Snapshot},
    repository_graph::{self, Edge, Graph, Node},
};
use std::{
    collections::{BTreeSet, HashMap},
    sync::Arc,
};

const HEADER: f32 = 58.0;
const PAD: f32 = 24.0;
const GAP: f32 = 46.0;
const LEAF: Vec2 = Vec2::new(234.0, 108.0);

pub struct View {
    pub expanded: BTreeSet<String>,
    pub zoom: f32,
    snapshot: Option<Arc<Snapshot>>,
    scene: Option<Scene>,
    revision: u64,
    built_revision: u64,
    reveal: Option<String>,
}

impl Default for View {
    fn default() -> Self {
        Self {
            expanded: BTreeSet::new(),
            zoom: 0.85,
            snapshot: None,
            scene: None,
            revision: 0,
            built_revision: 0,
            reveal: None,
        }
    }
}

impl View {
    pub fn reset(&mut self) {
        *self = Self::default();
    }
    pub fn layers(&mut self) {
        self.expanded.clear();
        self.revision += 1;
        self.reveal = Some(String::new());
    }
    pub fn slices(&mut self, snapshot: &Snapshot) {
        self.expanded = snapshot
            .tree
            .children
            .iter()
            .map(|node| node.path.clone())
            .collect();
        self.revision += 1;
        self.reveal = Some(String::new());
    }
    pub fn reveal(&mut self, tree: &Entry, path: &str) {
        repository_graph::reveal(tree, path, &mut self.expanded);
        self.revision += 1;
        self.reveal = Some(path.into());
    }
    fn toggle(&mut self, path: &str) {
        if !self.expanded.remove(path) {
            self.expanded.insert(path.into());
        }
        self.revision += 1;
        self.reveal = Some(path.into());
    }
    pub fn show(&mut self, ui: &mut egui::Ui, snapshot: &Arc<Snapshot>, selected: &mut String) {
        if self
            .snapshot
            .as_ref()
            .is_none_or(|old| !Arc::ptr_eq(old, snapshot))
            || self.built_revision != self.revision
        {
            self.scene = Some(Scene::new(repository_graph::project(
                snapshot,
                &self.expanded,
            )));
            self.snapshot = Some(snapshot.clone());
            self.built_revision = self.revision;
        }
        let scene = self.scene.as_ref().unwrap();
        ui.horizontal(|ui| {
            ui.label(
                egui::RichText::new(format!(
                    "노드 {} · import 관계 {}",
                    scene.boxes.len(),
                    scene.edges.len()
                ))
                .color(MUTED)
                .small(),
            );
            ui.label(
                egui::RichText::new("노드를 눌러 내부 펼치기 · 펼친 그룹의 제목을 누르면 접기")
                    .color(MUTED)
                    .small(),
            );
        });
        let mut toggle = None;
        let mut clicked = None;
        let scale = self.zoom;
        let requested = self.reveal.take();
        egui::ScrollArea::both()
            .id_salt("compound-repository-graph")
            .show(ui, |ui| {
                let size = scene.size * scale;
                let (canvas, response) = ui.allocate_exact_size(
                    Vec2::new(
                        size.x.max(ui.available_width()),
                        size.y.max(ui.available_height()),
                    ),
                    Sense::click(),
                );
                let origin = canvas.min;
                let transform = |rect: Rect| {
                    Rect::from_min_max(
                        origin + rect.min.to_vec2() * scale,
                        origin + rect.max.to_vec2() * scale,
                    )
                };
                let point = |p: Pos2| origin + p.to_vec2() * scale;
                let painter = ui.painter();
                let visible = canvas.intersect(ui.clip_rect());
                let step = 24.0 * scale;
                let start = origin
                    + egui::vec2(
                        ((visible.left() - canvas.left()) / step).floor() * step,
                        ((visible.top() - canvas.top()) / step).floor() * step,
                    );
                for x in 0..(visible.width() / step) as i32 + 2 {
                    for y in 0..(visible.height() / step) as i32 + 2 {
                        painter.circle_filled(
                            start + egui::vec2(x as f32 * step, y as f32 * step),
                            0.7,
                            BORDER.gamma_multiply(0.5),
                        );
                    }
                }
                // Keep all expanded boundaries behind both the child nodes and their import lines.
                for item in scene.boxes.iter().filter(|item| item.expanded) {
                    let rect = transform(item.rect);
                    if !ui.is_rect_visible(rect) {
                        continue;
                    }
                    let color = canvas::layer_color(&item.path);
                    let depth = item.depth.min(5) as u8;
                    painter.rect_filled(
                        rect,
                        12.0 * scale,
                        Color32::from_rgb(13 + depth * 3, 21 + depth * 3, 34 + depth * 4),
                    );
                    painter.rect_stroke(
                        rect,
                        12.0 * scale,
                        Stroke::new(
                            1.0,
                            if item.path == *selected {
                                ACCENT
                            } else {
                                color.gamma_multiply(0.45)
                            },
                        ),
                        StrokeKind::Inside,
                    );
                }
                // Paint unselected imports first so the selected group's relations stay legible.
                for active in [false, true] {
                    for route in &scene.edges {
                        let highlighted = !selected.is_empty()
                            && (inside(&route.edge.from, selected)
                                || inside(&route.edge.to, selected));
                        if highlighted != active {
                            continue;
                        }
                        let color = if highlighted {
                            ACCENT
                        } else if selected.is_empty() {
                            Color32::from_rgb(105, 129, 163)
                        } else {
                            Color32::from_rgb(55, 72, 98)
                        };
                        let points: Vec<_> = route.points.iter().copied().map(point).collect();
                        painter.add(egui::Shape::line(
                            points.clone(),
                            Stroke::new(if highlighted { 2.0 } else { 1.25 }, color),
                        ));
                        if let Some((end, previous)) = points.last().zip(points.iter().rev().nth(1))
                        {
                            let direction = (*end - *previous).normalized();
                            let perpendicular = egui::vec2(-direction.y, direction.x);
                            painter.add(egui::Shape::convex_polygon(
                                vec![
                                    *end,
                                    *end - direction * 8.0 * scale + perpendicular * 4.0 * scale,
                                    *end - direction * 8.0 * scale - perpendicular * 4.0 * scale,
                                ],
                                color,
                                Stroke::NONE,
                            ));
                        }
                    }
                }
                for item in &scene.boxes {
                    let rect = transform(item.rect);
                    let header = transform(item.header());
                    if requested.as_ref() == Some(&item.path) {
                        ui.scroll_to_rect(header, Some(egui::Align::Min));
                    }
                    if !ui.is_rect_visible(rect) {
                        continue;
                    }
                    let response = ui.interact(
                        header,
                        ui.scope_id().with((&item.path, "group-header")),
                        Sense::click(),
                    );
                    let label = format!(
                        "{} {}",
                        item.path,
                        if item.expandable {
                            if item.expanded { "접기" } else { "펼치기" }
                        } else {
                            "선택"
                        }
                    );
                    response.widget_info(|| {
                        egui::WidgetInfo::labeled(egui::Role::Button, ui.is_enabled(), &label)
                    });
                    let painter = ui.painter().with_clip_rect(rect.intersect(ui.clip_rect()));
                    let color = canvas::layer_color(&item.path);
                    if !item.expanded {
                        painter.rect_filled(
                            rect,
                            10.0 * scale,
                            if item.path == *selected {
                                Color32::from_rgb(38, 39, 67)
                            } else if response.hovered() {
                                Color32::from_rgb(30, 43, 65)
                            } else {
                                CARD
                            },
                        );
                        painter.rect_stroke(
                            rect,
                            10.0 * scale,
                            Stroke::new(
                                if item.path == *selected { 1.6 } else { 1.0 },
                                if item.path == *selected {
                                    ACCENT
                                } else {
                                    BORDER
                                },
                            ),
                            StrokeKind::Inside,
                        );
                    } else if response.hovered() {
                        painter.rect_filled(
                            header.shrink(2.0),
                            8.0 * scale,
                            color.gamma_multiply(0.09),
                        );
                    }
                    let start = rect.min + egui::vec2(16.0, 13.0) * scale;
                    let marker = if item.expandable {
                        if item.expanded { "▾" } else { "▸" }
                    } else {
                        "·"
                    };
                    painter.text(
                        start,
                        Align2::LEFT_TOP,
                        marker,
                        FontId::proportional(15.0 * scale),
                        color,
                    );
                    let title = if item.kind == "layer" {
                        item.name.to_uppercase()
                    } else {
                        item.name.clone()
                    };
                    let mut job = egui::text::LayoutJob::simple(
                        title,
                        FontId::proportional(16.0 * scale),
                        TEXT,
                        (rect.width() - 52.0 * scale).max(10.0),
                    );
                    job.wrap.max_rows = 1;
                    painter.galley(
                        start + egui::vec2(20.0, 0.0) * scale,
                        painter.layout_job(job),
                        TEXT,
                    );
                    let badge = format!(
                        "{}{} · 파일 {}",
                        kind_label(&item.kind),
                        if item.index { " · index" } else { "" },
                        item.file_count
                    );
                    painter.text(
                        start + egui::vec2(20.0, 25.0) * scale,
                        Align2::LEFT_TOP,
                        badge,
                        FontId::proportional(10.0 * scale),
                        color,
                    );
                    if !item.expanded {
                        let detail = if item.kind == "file" {
                            "실제 소스 파일 · 관계 보기"
                        } else {
                            "클릭하여 내부 그래프 펼치기"
                        };
                        painter.text(
                            rect.left_bottom() + egui::vec2(16.0, -22.0) * scale,
                            Align2::LEFT_TOP,
                            detail,
                            FontId::proportional(10.0 * scale),
                            MUTED,
                        );
                    }
                    if response.clicked() {
                        clicked = Some(item.path.clone());
                        if item.expandable && !ui.input(|input| input.modifiers.shift) {
                            toggle = Some(item.path.clone());
                        }
                    }
                    response.on_hover_text(format!(
                        "{}\n{}",
                        item.path,
                        if item.expandable {
                            "클릭: 펼치기/접기 · Shift+클릭: 관계만 선택"
                        } else {
                            "클릭: 파일의 import 관계 선택"
                        }
                    ));
                }
                // Import details remain on the graph itself rather than requiring the side inspector.
                for (i, route) in scene.edges.iter().enumerate() {
                    if selected.is_empty()
                        || (route.edge.from != *selected && route.edge.to != *selected)
                    {
                        continue;
                    }
                    let Some(anchor) = route.label_position() else {
                        continue;
                    };
                    let anchor = point(anchor);
                    let label = format!("{}", route.edge.file_pairs);
                    let label_rect = Rect::from_center_size(anchor, egui::vec2(24.0, 18.0) * scale);
                    if !ui.is_rect_visible(label_rect) {
                        continue;
                    }
                    let painter = ui.painter();
                    painter.rect_filled(label_rect, 4.0, BACKGROUND);
                    painter.text(
                        anchor,
                        Align2::CENTER_CENTER,
                        label,
                        FontId::proportional(10.0 * scale),
                        MUTED,
                    );
                    ui.interact(
                        label_rect,
                        ui.scope_id().with(("import-count", i)),
                        Sense::hover(),
                    )
                    .on_hover_text(format!(
                        "{} → {}\n파일 관계 {}개\n{}",
                        route.edge.from,
                        route.edge.to,
                        route.edge.file_pairs,
                        route.edge.names.join(", ")
                    ));
                }
                if let Some(pointer) = ui
                    .input(|input| input.pointer.hover_pos())
                    .filter(|pointer| visible.contains(*pointer))
                {
                    let closest = scene
                        .edges
                        .iter()
                        .enumerate()
                        .filter_map(|(i, route)| {
                            let distance = route
                                .points
                                .windows(2)
                                .map(|segment| {
                                    let a = point(segment[0]);
                                    let b = point(segment[1]);
                                    let vector = b - a;
                                    let t = ((pointer - a).dot(vector)
                                        / vector.length_sq().max(0.01))
                                    .clamp(0.0, 1.0);
                                    (pointer - (a + vector * t)).length_sq()
                                })
                                .min_by(f32::total_cmp)?;
                            Some((i, route, distance))
                        })
                        .min_by(|a, b| a.2.total_cmp(&b.2));
                    if let Some((i, route, _)) = closest.filter(|(_, _, distance)| *distance < 25.0)
                    {
                        ui.interact(
                            Rect::from_center_size(pointer, Vec2::splat(8.0)),
                            ui.scope_id().with(("import-line", i)),
                            Sense::hover(),
                        )
                        .on_hover_text(format!(
                            "{} → {}\n파일 관계 {}개\n{}",
                            route.edge.from,
                            route.edge.to,
                            route.edge.file_pairs,
                            route.edge.names.join(", ")
                        ));
                    }
                }
                if requested.as_deref() == Some("") {
                    ui.scroll_to_rect(
                        Rect::from_min_size(canvas.min, egui::vec2(1.0, 1.0)),
                        Some(egui::Align::Min),
                    );
                }
                if response.clicked() && clicked.is_none() {
                    selected.clear();
                }
            });
        if let Some(path) = clicked {
            *selected = path;
        }
        if let Some(path) = toggle {
            self.toggle(&path);
            ui.ctx().request_repaint();
        }
    }
}

pub fn kind_label(kind: &str) -> &str {
    match kind {
        "layer" => "LAYER",
        "slice" => "SLICE",
        "group" => "INDEX GROUP",
        "file" => "FILE",
        _ => "FOLDER",
    }
}

fn inside(path: &str, group: &str) -> bool {
    path == group || path.starts_with(&format!("{group}/"))
}

struct BoxItem {
    path: String,
    name: String,
    kind: String,
    index: bool,
    file_count: usize,
    rect: Rect,
    expanded: bool,
    expandable: bool,
    depth: usize,
}
impl BoxItem {
    fn header(&self) -> Rect {
        if self.expanded {
            Rect::from_min_size(self.rect.min, egui::vec2(self.rect.width(), HEADER))
        } else {
            self.rect
        }
    }
}

struct Branch<'a> {
    node: &'a Node,
    size: Vec2,
    children: Vec<(Pos2, Branch<'a>)>,
}

fn measure(node: &Node, budget: f32) -> Branch<'_> {
    if node.children.is_empty() {
        return Branch {
            node,
            size: if node.kind == "layer" {
                egui::vec2(budget.max(LEAF.x), 82.0)
            } else {
                LEAF
            },
            children: vec![],
        };
    }
    let available = (budget - PAD * 2.0).max(LEAF.x);
    let columns = ((available + GAP) / (LEAF.x + GAP)).floor().max(1.0) as usize;
    let child_budget = ((available - (columns - 1) as f32 * GAP) / columns as f32).max(LEAF.x);
    let mut children = vec![];
    let (mut x, mut y, mut row_height, mut width) =
        (PAD, HEADER + PAD, 0.0f32, available + PAD * 2.0);
    for child in &node.children {
        let branch = measure(child, child_budget);
        if x > PAD && x + branch.size.x > available + PAD + 0.1 {
            y += row_height + GAP;
            x = PAD;
            row_height = 0.0;
        }
        row_height = row_height.max(branch.size.y);
        width = width.max(x + branch.size.x + PAD);
        let child_width = branch.size.x;
        children.push((egui::pos2(x, y), branch));
        x += child_width + GAP;
    }
    Branch {
        node,
        size: egui::vec2(width, y + row_height + PAD),
        children,
    }
}

struct Route {
    edge: Edge,
    points: Vec<Pos2>,
}
impl Route {
    fn label_position(&self) -> Option<Pos2> {
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

struct Scene {
    boxes: Vec<BoxItem>,
    edges: Vec<Route>,
    size: Vec2,
}
impl Scene {
    fn new(graph: Graph) -> Self {
        fn flatten(branch: &Branch<'_>, origin: Pos2, depth: usize, boxes: &mut Vec<BoxItem>) {
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
            });
            for (offset, child) in &branch.children {
                flatten(child, origin + offset.to_vec2(), depth + 1, boxes);
            }
        }
        let mut boxes = vec![];
        let (mut y, mut width) = (28.0, 900.0f32);
        for node in &graph.nodes {
            let branch = measure(node, 900.0);
            flatten(&branch, egui::pos2(50.0, y), 0, &mut boxes);
            width = width.max(branch.size.x);
            y += branch.size.y + 62.0;
        }
        let size = egui::vec2(width + 190.0, y + 12.0);
        let by_path: HashMap<_, _> = boxes
            .iter()
            .map(|item| (item.path.as_str(), item))
            .collect();
        let endpoints: Vec<_> = graph
            .edges
            .iter()
            .filter_map(|edge| {
                let from = by_path.get(edge.from.as_str())?;
                let to = by_path.get(edge.to.as_str())?;
                Some(ports(from, to))
            })
            .collect();
        let obstacles: Vec<_> = boxes
            .iter()
            .map(|item| {
                if item.expanded {
                    item.header().shrink2(egui::vec2(2.0, 0.0))
                } else {
                    item.rect
                }
            })
            .collect();
        let mut router = crate::routing::Router::new(&obstacles, &endpoints, size);
        let edges = graph
            .edges
            .into_iter()
            .zip(endpoints)
            .map(|(edge, (a, start, end, b))| {
                let mut points = vec![a];
                points.extend(router.route(start, end));
                points.push(b);
                Route {
                    edge,
                    points: simplify(points),
                }
            })
            .collect();
        Self { boxes, edges, size }
    }
}

fn ports(from: &BoxItem, to: &BoxItem) -> (Pos2, Pos2, Pos2, Pos2) {
    let (a, normal_a, b, normal_b) = if from.kind == "layer" || to.kind == "layer" {
        (
            from.rect.right_center(),
            Vec2::X,
            to.rect.right_center(),
            Vec2::X,
        )
    } else if to.rect.top() > from.rect.bottom() + 4.0 {
        (
            from.rect.center_bottom(),
            Vec2::Y,
            to.rect.center_top(),
            -Vec2::Y,
        )
    } else if from.rect.top() > to.rect.bottom() + 4.0 {
        (
            from.rect.center_top(),
            -Vec2::Y,
            to.rect.center_bottom(),
            Vec2::Y,
        )
    } else if to.rect.center().x > from.rect.center().x {
        (
            from.rect.right_center(),
            Vec2::X,
            to.rect.left_center(),
            -Vec2::X,
        )
    } else {
        (
            from.rect.left_center(),
            -Vec2::X,
            to.rect.right_center(),
            Vec2::X,
        )
    };
    (a, a + normal_a * 12.0, b + normal_b * 12.0, b)
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

#[cfg(test)]
mod tests {
    use super::*;
    fn node(path: &str, kind: &str, children: Vec<Node>) -> Node {
        Node {
            path: path.into(),
            name: path.rsplit('/').next().unwrap().into(),
            kind: kind.into(),
            index: kind == "group",
            file_count: 1,
            expandable: !children.is_empty(),
            children,
        }
    }
    #[test]
    fn compound_layout_encloses_children_and_routes_all_imports_outside_nodes_and_titles() {
        let add = "features/cart/model/actions/add.ts";
        let index = "features/cart/model/actions/index.ts";
        let graph = Graph {
            nodes: vec![
                node(
                    "features",
                    "layer",
                    vec![
                        node(
                            "features/cart",
                            "slice",
                            vec![node(
                                "features/cart/model",
                                "folder",
                                vec![node(
                                    "features/cart/model/actions",
                                    "group",
                                    vec![node(add, "file", vec![]), node(index, "file", vec![])],
                                )],
                            )],
                        ),
                        node("features/checkout", "slice", vec![]),
                        node("features/order", "slice", vec![]),
                    ],
                ),
                node("shared", "layer", vec![]),
            ],
            edges: [
                (index, add),
                (add, "shared"),
                ("features/checkout", add),
                ("features/order", "shared"),
            ]
            .into_iter()
            .map(|(from, to)| Edge {
                from: from.into(),
                to: to.into(),
                file_pairs: 1,
                names: vec![],
            })
            .collect(),
        };
        let scene = Scene::new(graph);
        assert_eq!(scene.edges.len(), 4);
        for child in &scene.boxes {
            if let Some((parent, _)) = child.path.rsplit_once('/') {
                let parent = scene.boxes.iter().find(|item| item.path == parent).unwrap();
                assert!(parent.rect.contains_rect(child.rect));
                assert!(child.rect.top() >= parent.header().bottom());
            }
        }
        for route in &scene.edges {
            for segment in route.points.windows(2) {
                let (a, b) = (segment[0], segment[1]);
                assert!(a.x == b.x || a.y == b.y);
                for item in &scene.boxes {
                    let rect = item.header().shrink(0.1);
                    let crosses = if a.x == b.x {
                        a.x > rect.left()
                            && a.x < rect.right()
                            && a.y.max(b.y) > rect.top()
                            && a.y.min(b.y) < rect.bottom()
                    } else {
                        a.y > rect.top()
                            && a.y < rect.bottom()
                            && a.x.max(b.x) > rect.left()
                            && a.x.min(b.x) < rect.right()
                    };
                    assert!(
                        !crosses,
                        "{} -> {} crosses {}",
                        route.edge.from, route.edge.to, item.path
                    );
                }
            }
        }
    }
}
