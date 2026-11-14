use crate::canvas::{self, ACCENT, BACKGROUND, BORDER, CARD, MUTED, TEXT};
use eframe::egui::{self, Align2, Color32, FontId, Pos2, Rect, Sense, Stroke, StrokeKind, Vec2};
use factory_core::{
    repository::{Entry, Snapshot},
    repository_graph::{self, Edge, Graph, Node},
};
use std::{
    collections::{BTreeSet, HashMap},
    sync::{Arc, mpsc},
    time::{Duration, Instant},
};

#[path = "layout_comparison.rs"]
mod comparison;
pub use comparison::run as compare;

const HEADER: f32 = 58.0;
const PAD: f32 = 36.0;
const GAP: f32 = 48.0;
const LEAF: Vec2 = Vec2::new(234.0, 108.0);
const FOCUS_GAP: f32 = 180.0;

struct Request {
    id: u64,
    graph: Graph,
    selected: String,
}
struct Response {
    id: u64,
    result: Result<Scene, String>,
    elapsed: Duration,
}
struct Worker {
    send: mpsc::Sender<Request>,
    receive: mpsc::Receiver<Response>,
}
impl Worker {
    fn new(ctx: egui::Context) -> Self {
        let (send, requests) = mpsc::channel::<Request>();
        let (responses, receive) = mpsc::channel();
        std::thread::spawn(move || {
            while let Ok(mut request) = requests.recv() {
                // Expansion clicks queued during a layout only need the newest graph.
                while let Ok(latest) = requests.try_recv() {
                    request = latest;
                }
                let start = Instant::now();
                let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    Scene::focused(request.graph, &request.selected)
                }))
                .unwrap_or_else(|_| Err("레이아웃 계산 중 오류가 발생했습니다.".into()));
                if responses
                    .send(Response {
                        id: request.id,
                        result,
                        elapsed: start.elapsed(),
                    })
                    .is_err()
                {
                    break;
                }
                ctx.request_repaint();
            }
        });
        Self { send, receive }
    }
}

pub struct View {
    pub expanded: BTreeSet<String>,
    pub zoom: f32,
    snapshot: Option<Arc<Snapshot>>,
    scene: Option<Scene>,
    revision: u64,
    built_revision: u64,
    reveal: Option<String>,
    worker: Option<Worker>,
    request: u64,
    pending: bool,
    elapsed: Duration,
    error: Option<String>,
    fit_on_complete: bool,
    routed_selection: String,
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
            reveal: Some(String::new()),
            worker: None,
            request: 0,
            pending: false,
            elapsed: Duration::ZERO,
            error: None,
            fit_on_complete: true,
            routed_selection: String::new(),
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
        self.fit_on_complete = true;
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
        if is_layer(selected) {
            selected.clear();
        }
        let mut fit = false;
        ui.horizontal_wrapped(|ui| {
            fit = ui.button("화면 맞춤").clicked();
            if ui
                .add_enabled(!selected.is_empty(), egui::Button::new("선택 해제"))
                .clicked()
            {
                selected.clear();
            }
            if self.pending {
                ui.spinner();
            }
        });
        let geometry_changed = self
            .snapshot
            .as_ref()
            .is_none_or(|old| !Arc::ptr_eq(old, snapshot))
            || self.built_revision != self.revision;
        if geometry_changed || self.routed_selection != *selected {
            if geometry_changed {
                self.scene = None;
            }
            self.error = None;
            self.pending = true;
            self.request += 1;
            let worker = self
                .worker
                .get_or_insert_with(|| Worker::new(ui.ctx().clone()));
            if worker
                .send
                .send(Request {
                    id: self.request,
                    graph: repository_graph::project(snapshot, &self.expanded),
                    selected: selected.clone(),
                })
                .is_err()
            {
                self.error = Some("레이아웃 작업을 시작하지 못했습니다.".into());
                self.pending = false;
            }
            self.snapshot = Some(snapshot.clone());
            self.built_revision = self.revision;
        }
        self.routed_selection = selected.clone();
        if let Some(worker) = &self.worker {
            while let Ok(response) = worker.receive.try_recv() {
                if response.id != self.request {
                    continue;
                }
                self.pending = false;
                self.elapsed = response.elapsed;
                match response.result {
                    Ok(scene) => {
                        self.scene = Some(scene);
                        self.error = None;
                    }
                    Err(error) => {
                        self.scene = None;
                        self.error = Some(error);
                    }
                }
            }
        }
        if let Some(error) = &self.error {
            ui.colored_label(egui::Color32::LIGHT_RED, error);
            return;
        }
        let Some(scene) = self.scene.as_ref() else {
            ui.label("중첩 그래프와 연결선을 배치하고 있습니다…");
            return;
        };
        if fit || self.fit_on_complete {
            self.zoom = ((ui.available_width() - 24.0) / scene.size.x)
                .clamp(canvas::GRAPH_ZOOM_MIN, canvas::GRAPH_ZOOM_MAX);
            self.fit_on_complete = false;
        }
        ui.horizontal_wrapped(|ui| {
            ui.label(
                egui::RichText::new(format!(
                    "ELK Libavoid · {:.0} ms",
                    self.elapsed.as_secs_f64() * 1000.0
                ))
                .color(MUTED)
                .small(),
            );
            let shown = scene
                .edges
                .iter()
                .filter(|route| scene.shows(&route.edge, selected))
                .count();
            ui.label(
                egui::RichText::new(format!(
                    "노드 {} · 선택 연결 {} / {}",
                    scene.boxes.len(),
                    shown,
                    scene.edges.len()
                ))
                .color(MUTED)
                .small(),
            );
            ui.label(
                egui::RichText::new(
                    "블록 클릭: 연결 보기 · ▸ / ▾: 펼치기 / 접기 · 드래그: 이동 · Ctrl+휠: 확대 / 축소",
                )
                .color(MUTED)
                .small(),
            );
            if !selected.is_empty() {
                ui.label(egui::RichText::new("→ 사용").color(ACCENT).small());
                ui.label(
                    egui::RichText::new("← 사용됨")
                        .color(Color32::from_rgb(100, 207, 189))
                        .small(),
                );
            }
        });
        if selected.is_empty() {
            ui.label(
                egui::RichText::new(
                    "블록을 클릭하면 그 블록이 사용하는 곳과 사용되는 곳의 연결만 표시합니다.",
                )
                .color(MUTED)
                .small(),
            );
        } else {
            ui.label(
                egui::RichText::new(format!("선택: {selected}"))
                    .color(MUTED)
                    .small(),
            );
        }
        let mut toggle = None;
        let mut clicked = None;
        let zoom_offset = canvas::zoom_at_pointer(ui, &mut self.zoom, "compound-repository-graph");
        if zoom_offset.is_some() {
            self.reveal = None;
        }
        let scale = self.zoom;
        let requested = if self.pending {
            None
        } else {
            self.reveal.take()
        };
        let mut scroll = egui::ScrollArea::both()
            .id_salt("compound-repository-graph")
            .auto_shrink([false, false])
            .scroll_source(egui::scroll_area::ScrollSource::ALL)
            .on_drag_cursor(egui::CursorIcon::Grabbing);
        if let Some(offset) = zoom_offset {
            scroll = scroll.scroll_offset(offset);
        }
        scroll.show(ui, |ui| {
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
                let mut painter = painter.clone();
                if item.dimmed {
                    painter.multiply_opacity(0.24);
                }
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
            for route in &scene.edges {
                let Some(relation) = relation(&route.edge, selected) else {
                    continue;
                };
                if route.points.len() < 2 {
                    continue;
                }
                let color = if relation == Direction::Incoming {
                    Color32::from_rgb(100, 207, 189)
                } else {
                    ACCENT
                };
                let points: Vec<_> = route.points.iter().copied().map(point).collect();
                // A dark gap at a crossing distinguishes an overpass from a junction.
                painter.add(egui::Shape::line(
                    points.clone(),
                    Stroke::new(6.0, BACKGROUND),
                ));
                painter.add(egui::Shape::line(points.clone(), Stroke::new(2.0, color)));
                if let Some((end, previous)) = points.last().zip(points.iter().rev().nth(1)) {
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
                let label = if item.kind == "layer" {
                    format!(
                        "{} 레이어 {}",
                        item.path,
                        if item.expanded { "접기" } else { "펼치기" }
                    )
                } else {
                    format!("{} 선택", item.path)
                };
                response.widget_info(|| {
                    egui::WidgetInfo::labeled(egui::Role::Button, ui.is_enabled(), &label)
                });
                let mut painter = ui.painter().with_clip_rect(rect.intersect(ui.clip_rect()));
                if item.dimmed {
                    painter.multiply_opacity(0.24);
                }
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
                if !item.expandable {
                    painter.text(
                        start,
                        Align2::LEFT_TOP,
                        marker,
                        FontId::proportional(15.0 * scale),
                        color,
                    );
                }
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
                    let outgoing = scene
                        .edges
                        .iter()
                        .filter(|route| route.edge.from == item.path)
                        .count();
                    let incoming = scene
                        .edges
                        .iter()
                        .filter(|route| route.edge.to == item.path)
                        .count();
                    let detail = format!("사용 {} · 사용됨 {}", outgoing, incoming);
                    painter.text(
                        rect.left_bottom() + egui::vec2(16.0, -22.0) * scale,
                        Align2::LEFT_TOP,
                        &detail,
                        FontId::proportional(10.0 * scale),
                        MUTED,
                    );
                }
                let expansion_button = item.expandable.then(|| {
                    let button_rect = Rect::from_min_size(
                        rect.min + egui::vec2(9.0, 8.0) * scale,
                        egui::vec2(24.0, 27.0) * scale,
                    );
                    let mut button_ui = ui.new_child(egui::UiBuilder::new().max_rect(button_rect));
                    if item.dimmed {
                        button_ui.multiply_opacity(0.24);
                    }
                    let response = button_ui.put(
                        button_rect,
                        egui::Button::new(
                            egui::RichText::new(marker).size(15.0 * scale).color(color),
                        )
                        .frame(false),
                    );
                    let action = if item.expanded { "접기" } else { "펼치기" };
                    response.widget_info(|| {
                        egui::WidgetInfo::labeled(
                            egui::Role::Button,
                            ui.is_enabled(),
                            format!("{} {action}", item.path),
                        )
                    });
                    response.on_hover_text(action)
                });
                let expanding = expansion_button
                    .as_ref()
                    .is_some_and(|response| response.clicked());
                if expanding {
                    toggle = Some(item.path.clone());
                }
                if response.clicked() && !expanding {
                    if item.kind == "layer" {
                        toggle = Some(item.path.clone());
                    } else {
                        clicked = Some(item.path.clone());
                    }
                }
                response.on_hover_text(format!(
                    "{}\n{}",
                    item.path,
                    if item.kind == "layer" {
                        "클릭: 레이어 펼치기 / 접기"
                    } else if item.expandable {
                        "클릭: 이 블록의 연결 보기 · ▸ / ▾ 버튼: 펼치기 / 접기"
                    } else {
                        "클릭: 파일의 import 관계 선택"
                    }
                ));
            }
            // Import details remain on the graph itself rather than requiring the side inspector.
            for (i, route) in scene.edges.iter().enumerate() {
                if !scene.shows(&route.edge, selected) {
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
                    .filter(|(_, route)| scene.shows(&route.edge, selected))
                    .filter_map(|(i, route)| {
                        let distance = route
                            .points
                            .windows(2)
                            .map(|segment| {
                                let a = point(segment[0]);
                                let b = point(segment[1]);
                                let vector = b - a;
                                let t = ((pointer - a).dot(vector) / vector.length_sq().max(0.01))
                                    .clamp(0.0, 1.0);
                                (pointer - (a + vector * t)).length_sq()
                            })
                            .min_by(f32::total_cmp)?;
                        Some((i, route, distance))
                    })
                    .min_by(|a, b| a.2.total_cmp(&b.2));
                if let Some((i, route, _)) = closest.filter(|(_, _, distance)| *distance < 25.0) {
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
                ui.ctx().request_repaint();
            }
        });
        if let Some(path) = clicked {
            if !path.is_empty() {
                self.reveal = Some(path.clone());
            }
            *selected = path;
            ui.ctx().request_repaint();
        }
        if let Some(path) = toggle {
            if self.expanded.contains(&path) && inside(selected, &path) {
                *selected = if is_layer(&path) {
                    String::new()
                } else {
                    path.clone()
                };
            }
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

fn is_layer(path: &str) -> bool {
    factory_core::repository::LAYERS.contains(&path)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Direction {
    Incoming,
    Outgoing,
}

// For an expanded block, its descendants still represent the selected block's boundary.
// Internal edges and transitive dependencies do not belong to that boundary.
fn relation(edge: &Edge, selected: &str) -> Option<Direction> {
    if selected.is_empty() || is_layer(selected) {
        return None;
    }
    match (inside(&edge.from, selected), inside(&edge.to, selected)) {
        (true, false) => Some(Direction::Outgoing),
        (false, true) => Some(Direction::Incoming),
        _ => None,
    }
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
    dimmed: bool,
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
    active_end: f32,
}

struct Focus {
    selected: String,
    related: BTreeSet<String>,
}
impl Focus {
    fn new(graph: &Graph, selected: &str) -> Self {
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
    fn dimmed(&self, path: &str) -> bool {
        !self.selected.is_empty() && !is_layer(path) && !self.related.contains(path)
    }
    fn rank(&self, path: &str) -> u8 {
        if self.selected.is_empty() || inside(&self.selected, path) {
            0
        } else if self.related.contains(path) {
            1
        } else {
            2
        }
    }
}

fn measure<'a>(node: &'a Node, focus: &Focus, dock: Option<f32>) -> Branch<'a> {
    if node.children.is_empty() {
        let size = if node.kind == "layer" {
            egui::vec2(1050.0, 82.0)
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
        children.push((egui::pos2(x, y), branch));
        x += child_width + GAP;
    }
    Branch {
        node,
        size: egui::vec2(x - GAP + PAD, y + height + PAD),
        children,
        active_end,
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
struct RouteInput {
    ids: Vec<usize>,
    connections: Vec<(usize, usize)>,
    ports: Vec<(Pos2, Pos2, Pos2, Pos2)>,
    obstacles: Vec<Rect>,
}
impl Scene {
    fn from_elk(graph: Graph, layout: &crate::layout_engine::Layout) -> Self {
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
    fn shows(&self, edge: &Edge, selected: &str) -> bool {
        relation(edge, selected).is_some()
    }
    fn new(graph: Graph) -> Self {
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
    fn focused(graph: Graph, selected: &str) -> Result<Self, String> {
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
        let shift = egui::vec2((-bounds.left()).max(0.0), (-bounds.top()).max(0.0));
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
    fn arrange(graph: Graph, selected: &str) -> Self {
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
            flatten(&branch, egui::pos2(50.0, y), 0, &focus, &mut boxes);
            width = width.max(branch.size.x);
            y += branch.size.y + 90.0;
        }
        let size = egui::vec2(width + 100.0, y + 32.0);
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
                    item.header().shrink2(egui::vec2(2.0, 0.0))
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
    fn fifty_slices_stay_on_one_row_without_wrapping() {
        let graph = Graph {
            nodes: vec![node(
                "features",
                "layer",
                (0..50)
                    .map(|i| node(&format!("features/slice-{i:02}"), "slice", vec![]))
                    .collect(),
            )],
            edges: vec![],
        };
        let scene = Scene::focused(graph, "").unwrap();
        let slices: Vec<_> = scene
            .boxes
            .iter()
            .filter(|item| item.kind == "slice")
            .collect();
        assert_eq!(slices.len(), 50);
        assert!(
            slices
                .iter()
                .all(|item| item.rect.top() == slices[0].rect.top())
        );
        assert!(
            slices
                .windows(2)
                .all(|items| items[0].rect.right() < items[1].rect.left())
        );
    }
    #[test]
    fn one_row_focus_docks_unrelated_cards_and_disables_layer_relations() {
        let graph = Graph {
            nodes: vec![
                node(
                    "features",
                    "layer",
                    vec![
                        node("features/history", "slice", vec![]),
                        node("features/cart", "slice", vec![]),
                        node("features/checkout", "slice", vec![]),
                    ],
                ),
                node("shared", "layer", vec![]),
            ],
            edges: [
                ("features/cart", "features/checkout"),
                ("features/cart", "shared"),
                ("features/checkout", "shared"),
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
        let scene = Scene::focused(graph.clone(), "features/cart").unwrap();
        assert_eq!(
            scene
                .edges
                .iter()
                .filter(|route| scene.shows(&route.edge, "features"))
                .count(),
            0
        );
        assert_eq!(
            scene
                .edges
                .iter()
                .filter(|route| scene.shows(&route.edge, "features/cart"))
                .count(),
            2
        );
        assert_eq!(
            scene
                .edges
                .iter()
                .filter(|route| scene.shows(&route.edge, ""))
                .count(),
            0
        );
        assert!(scene.edges[2].points.is_empty()); // Unrelated checkout → shared is hidden.
        assert_eq!(
            scene
                .edges
                .iter()
                .filter(|route| !route.points.is_empty())
                .count(),
            2
        );
        let other = Scene::focused(graph.clone(), "features/checkout").unwrap();
        assert!(other.edges[1].points.is_empty()); // Unrelated cart → shared must not be routed.
        assert_eq!(
            other
                .edges
                .iter()
                .filter(|route| !route.points.is_empty())
                .count(),
            2
        );
        assert_eq!(
            relation(&other.edges[0].edge, "features/checkout"),
            Some(Direction::Incoming)
        );
        assert_eq!(
            relation(&other.edges[1].edge, "features/cart"),
            Some(Direction::Outgoing)
        );
        let empty = Scene::focused(graph, "").unwrap();
        assert!(empty.edges.iter().all(|route| route.points.is_empty()));
        for candidate in [&other, &empty] {
            assert_eq!(scene.edges.len(), candidate.edges.len()); // All import facts survive selection.
            assert_eq!(
                scene
                    .boxes
                    .iter()
                    .map(|item| (&item.path, item.rect.top()))
                    .collect::<std::collections::BTreeMap<_, _>>(),
                candidate
                    .boxes
                    .iter()
                    .map(|item| (&item.path, item.rect.top()))
                    .collect::<std::collections::BTreeMap<_, _>>()
            );
        }
        let item = |scene: &Scene, path: &str| {
            scene
                .boxes
                .iter()
                .find(|item| item.path == path)
                .map(|item| (item.rect, item.dimmed))
                .unwrap()
        };
        assert!(item(&scene, "features/history").1);
        assert!(!item(&scene, "features/cart").1);
        assert!(!item(&scene, "features/checkout").1);
        assert!(
            item(&scene, "features/history").0.left()
                > item(&scene, "features/checkout").0.right() + FOCUS_GAP
        );
        assert!(
            item(&scene, "features/history").0.left() > item(&empty, "features/history").0.left()
        );
        assert_eq!(
            item(&scene, "features/history").0.top(),
            item(&scene, "features/cart").0.top()
        );
        assert!(empty.boxes.iter().all(|item| !item.dimmed));
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
        let elk = crate::layout_engine::layout(&graph).unwrap();
        let arranged = Scene::arrange(graph.clone(), add);
        let routed = Scene::focused(graph.clone(), add).unwrap();
        let offset = routed.boxes[0].rect.min - arranged.boxes[0].rect.min;
        for (before, after) in arranged.boxes.iter().zip(&routed.boxes) {
            assert_eq!(before.rect.translate(offset), after.rect); // Native ELK routing never rearranges cards.
        }
        let scenes = [
            Scene::from_elk(graph.clone(), &elk),
            Scene::new(graph),
            routed,
        ];
        for scene in scenes {
            assert_eq!(scene.edges.len(), 4);
            for (i, a) in scene.boxes.iter().enumerate() {
                for b in scene.boxes.iter().skip(i + 1) {
                    if inside(&a.path, &b.path) || inside(&b.path, &a.path) {
                        continue;
                    }
                    assert!(
                        !a.rect.shrink(0.1).intersects(b.rect.shrink(0.1)),
                        "{} overlaps {}",
                        a.path,
                        b.path
                    );
                }
            }
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
}
