use crate::{
    canvas::{self, ACCENT, BACKGROUND, BORDER, MUTED, PANEL, TEXT},
    repository_graph,
    storage::{self, Session},
    transport,
};
use eframe::egui::{self, Color32, RichText};
use factory_core::{
    plan::{Edge, Interface, Node, Plan},
    repository::{self, Entry, Snapshot},
};
use std::{
    path::PathBuf,
    sync::{Arc, mpsc},
    time::{Duration, Instant},
};

enum Event {
    Repository(Result<Snapshot, String>, bool),
    Import(Result<Option<Plan>, String>),
    Export(Result<Option<PathBuf>, String>),
}

pub struct Workbench {
    session: Session,
    repository: Option<Arc<Snapshot>>,
    repository_graph: repository_graph::View,
    inspected_path: String,
    search: String,
    planning: bool,
    selected: Option<String>,
    history: Vec<Plan>,
    redo: Vec<Plan>,
    last_edit: Instant,
    dirty: bool,
    storage_enabled: bool,
    busy: bool,
    message: String,
    error: bool,
    sender: mpsc::Sender<Event>,
    receiver: mpsc::Receiver<Event>,
    connection_open: bool,
    token: String,
    remember_token: bool,
    remote: bool,
    zoom: f32,
    connect_to: String,
    history_action: bool,
}

impl Workbench {
    pub fn new(cc: &eframe::CreationContext<'_>, initial: Option<PathBuf>) -> Self {
        let mut style = (*cc.egui_ctx.style_of(egui::Theme::Dark)).clone();
        style.visuals = egui::Visuals::dark();
        style.visuals.panel_fill = PANEL;
        style.visuals.window_fill = PANEL;
        style.visuals.extreme_bg_color = BACKGROUND;
        style.visuals.faint_bg_color = canvas::CARD;
        style.visuals.override_text_color = Some(TEXT);
        style.visuals.selection.bg_fill = Color32::from_rgb(60, 47, 102);
        style.visuals.selection.stroke.color = ACCENT;
        style.visuals.widgets.inactive.bg_fill = Color32::from_rgb(26, 37, 55);
        style.visuals.widgets.inactive.bg_stroke = egui::Stroke::new(1.0, BORDER);
        style.visuals.widgets.hovered.bg_fill = Color32::from_rgb(38, 54, 77);
        style.spacing.item_spacing = egui::vec2(9.0, 9.0);
        style.spacing.button_padding = egui::vec2(12.0, 8.0);
        style.spacing.interact_size.y = 31.0;
        style
            .text_styles
            .insert(egui::TextStyle::Body, egui::FontId::proportional(13.0));
        style
            .text_styles
            .insert(egui::TextStyle::Button, egui::FontId::proportional(12.0));
        cc.egui_ctx.set_style_of(egui::Theme::Dark, style);
        cc.egui_ctx.set_theme(egui::ThemePreference::Dark);
        // Accept Ctrl+wheel on both platforms, as well as the standard macOS Cmd+wheel.
        cc.egui_ctx.options_mut(|options| {
            options.input_options.zoom_modifier = egui::Modifiers {
                ctrl: true,
                command: true,
                ..Default::default()
            };
        });
        let (session, message, storage_enabled) = match storage::load() {
            Ok(session) => (
                session,
                "저장소를 열어 구조를 살펴보거나 직접 계획을 작성하세요.".into(),
                true,
            ),
            Err(error) => (Session::default(), error, false),
        };
        let (sender, receiver) = mpsc::channel();
        let mut app = Self {
            session,
            repository: None,
            repository_graph: repository_graph::View::default(),
            inspected_path: String::new(),
            search: String::new(),
            planning: false,
            selected: None,
            history: vec![],
            redo: vec![],
            last_edit: Instant::now(),
            dirty: false,
            storage_enabled,
            busy: false,
            message,
            error: !storage_enabled,
            sender,
            receiver,
            connection_open: false,
            token: String::new(),
            remember_token: false,
            remote: false,
            zoom: 1.0,
            connect_to: String::new(),
            history_action: false,
        };
        let path = initial.or_else(|| {
            (!app.session.repository.is_empty()).then(|| PathBuf::from(&app.session.repository))
        });
        if let Some(path) = path {
            app.open_repository(cc.egui_ctx.clone(), Some(path));
        }
        app
    }

    fn notify(&mut self, message: impl Into<String>, error: bool) {
        self.message = message.into();
        self.error = error;
    }

    fn open_repository(&mut self, ctx: egui::Context, path: Option<PathBuf>) {
        if self.busy {
            return;
        }
        self.busy = true;
        self.notify("저장소의 디렉터리와 import를 읽는 중…", false);
        let sender = self.sender.clone();
        std::thread::spawn(move || {
            let path = path.or_else(|| {
                rfd::FileDialog::new()
                    .set_title("FSD 저장소 열기")
                    .pick_folder()
            });
            if let Some(path) = path {
                let result = repository::analyze(&path);
                let _ = sender.send(Event::Repository(result, false));
            } else {
                let _ = sender.send(Event::Export(Ok(None)));
            }
            ctx.request_repaint();
        });
    }

    fn remote_repository(&mut self, ctx: egui::Context) {
        if self.busy {
            return;
        }
        self.busy = true;
        self.notify("서버의 현재 저장소 구조를 읽는 중…", false);
        let (server, token, remember) = (
            self.session.server.clone(),
            self.token.clone(),
            self.remember_token,
        );
        let sender = self.sender.clone();
        std::thread::spawn(move || {
            let result = transport::repository(&server, &token).and_then(|snapshot| {
                if remember {
                    transport::token(false, &token)?;
                }
                Ok(snapshot)
            });
            let _ = sender.send(Event::Repository(result, true));
            ctx.request_repaint();
        });
    }

    fn import(&mut self, ctx: egui::Context) {
        if self.busy {
            return;
        }
        self.busy = true;
        let sender = self.sender.clone();
        std::thread::spawn(move || {
            let result = rfd::FileDialog::new()
                .add_filter("계획 JSON", &["json"])
                .pick_file()
                .map(|file| {
                    std::fs::read_to_string(file)
                        .map_err(|e| e.to_string())
                        .and_then(|text| Plan::from_json(&text))
                })
                .transpose();
            let _ = sender.send(Event::Import(result));
            ctx.request_repaint();
        });
    }

    fn export(&mut self, ctx: egui::Context, markdown: bool) {
        if self.busy {
            return;
        }
        let text = match self.session.plan.json() {
            Ok(json) => {
                if markdown {
                    self.session.plan.markdown()
                } else {
                    json
                }
            }
            Err(error) => {
                self.notify(error, true);
                return;
            }
        };
        self.busy = true;
        let sender = self.sender.clone();
        std::thread::spawn(move || {
            let (extension, name) = if markdown {
                ("md", "implementation-plan.md")
            } else {
                ("json", "plan.json")
            };
            let result = rfd::FileDialog::new()
                .set_file_name(name)
                .add_filter("계획", &[extension])
                .save_file()
                .map(|file| {
                    std::fs::write(&file, text)
                        .map(|_| file)
                        .map_err(|e| e.to_string())
                })
                .transpose();
            let _ = sender.send(Event::Export(result));
            ctx.request_repaint();
        });
    }

    fn receive(&mut self) {
        while let Ok(event) = self.receiver.try_recv() {
            self.busy = false;
            match event {
                Event::Repository(Ok(snapshot), remote) => {
                    if !remote {
                        self.session.repository = snapshot.repository.clone();
                    }
                    self.notify(
                        format!(
                            "{} · 파일 {}개 · 추적 못 한 import {}건",
                            snapshot.source_root,
                            snapshot.tree.file_count,
                            snapshot.scan["analysis"]["unresolvedImports"]
                        ),
                        false,
                    );
                    self.repository = Some(Arc::new(snapshot));
                    self.remote = remote;
                    self.repository_graph.reset();
                    self.inspected_path.clear();
                    self.search.clear();
                    self.planning = false;
                    self.connection_open = false;
                    self.dirty = true;
                }
                Event::Repository(Err(error), _)
                | Event::Import(Err(error))
                | Event::Export(Err(error)) => self.notify(error, true),
                Event::Import(Ok(Some(plan))) => {
                    self.history.push(self.session.plan.clone());
                    self.redo.clear();
                    self.session.plan = plan;
                    self.selected = None;
                    self.planning = true;
                    self.dirty = true;
                    self.notify(
                        "계획을 불러왔습니다. 되돌리기로 이전 계획을 복구할 수 있습니다.",
                        false,
                    );
                }
                Event::Export(Ok(Some(file))) => {
                    self.notify(format!("내보냈습니다: {}", file.display()), false)
                }
                _ => self.notify("취소했습니다.", false),
            }
        }
    }

    fn undo(&mut self, redo: bool) {
        self.history_action = true;
        let (source, target) = if redo {
            (&mut self.redo, &mut self.history)
        } else {
            (&mut self.history, &mut self.redo)
        };
        if let Some(plan) = source.pop() {
            target.push(std::mem::replace(&mut self.session.plan, plan));
            self.dirty = true;
            self.last_edit = Instant::now();
            if !self
                .session
                .plan
                .nodes
                .iter()
                .any(|node| Some(&node.id) == self.selected.as_ref())
            {
                self.selected = None;
            }
        }
    }

    fn add_module(&mut self) {
        let node = Node::default();
        self.selected = Some(node.id.clone());
        self.session.plan.nodes.push(node);
        self.planning = true;
    }

    fn add_entry(&mut self, snapshot: &Snapshot, entry: &Entry) {
        let node = snapshot.module(entry);
        let check = Plan {
            nodes: vec![node.clone()],
            ..Default::default()
        };
        if !check.errors().is_empty() {
            self.notify(
                "슬라이스 내부의 세그먼트·그룹·파일을 선택한 뒤 계획에 추가하세요.",
                true,
            );
            return;
        }
        if self
            .session
            .plan
            .nodes
            .iter()
            .any(|old| old.path == node.path && old.files == node.files)
        {
            self.notify("이 그룹은 이미 계획에 있습니다.", false);
            return;
        }
        self.selected = Some(node.id.clone());
        self.session.plan.nodes.push(node);
        self.planning = true;
        self.notify(
            "실제 파일 목록을 계획에 복사했습니다. 책임과 변경할 내용을 직접 작성하세요.",
            false,
        );
    }

    fn header(&mut self, ui: &mut egui::Ui) {
        egui::Panel::top("header")
            .frame(
                egui::Frame::new()
                    .fill(PANEL)
                    .inner_margin(egui::Margin::symmetric(22, 15)),
            )
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    egui::Frame::new()
                        .fill(ACCENT)
                        .corner_radius(11)
                        .inner_margin(10)
                        .show(ui, |ui| {
                            ui.label(RichText::new("⌘").size(23.0).color(BACKGROUND));
                        });
                    ui.vertical(|ui| {
                        ui.label(RichText::new("Planning Workbench").size(18.0).strong());
                        ui.label(
                            RichText::new(format!(
                                "계획 v{} · 모듈 {} · 관계 {}",
                                self.session.plan.version,
                                self.session.plan.nodes.len(),
                                self.session.plan.edges.len()
                            ))
                            .size(11.0)
                            .color(MUTED),
                        );
                    });
                    ui.add_space(20.0);
                    if ui.selectable_label(!self.planning, "저장소").clicked() {
                        self.planning = false;
                    }
                    if ui.selectable_label(self.planning, "계획").clicked() {
                        self.planning = true;
                    }
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.menu_button("내보내기", |ui| {
                            if ui.button("JSON · 다시 편집할 계획").clicked() {
                                self.export(ui.ctx().clone(), false);
                                ui.close();
                            }
                            if ui.button("Markdown · AI에 전달할 계획").clicked() {
                                self.export(ui.ctx().clone(), true);
                                ui.close();
                            }
                            if ui.button("Markdown 복사").clicked() {
                                match self.session.plan.json() {
                                    Ok(_) => {
                                        ui.ctx().copy_text(self.session.plan.markdown());
                                        self.notify("계획을 클립보드에 복사했습니다.", false);
                                    }
                                    Err(error) => self.notify(error, true),
                                }
                                ui.close();
                            }
                        });
                        if ui
                            .add_enabled(!self.busy, egui::Button::new("JSON 불러오기"))
                            .clicked()
                        {
                            self.import(ui.ctx().clone());
                        }
                        if self.planning {
                            if ui.button("＋ 모듈").clicked() {
                                self.add_module();
                            }
                            if ui
                                .add_enabled(!self.redo.is_empty(), egui::Button::new("↷"))
                                .on_hover_text("다시 실행")
                                .clicked()
                            {
                                self.undo(true);
                            }
                            if ui
                                .add_enabled(!self.history.is_empty(), egui::Button::new("↶"))
                                .on_hover_text("되돌리기")
                                .clicked()
                            {
                                self.undo(false);
                            }
                        }
                    });
                });
            });
        egui::Panel::top("context")
            .frame(
                egui::Frame::new()
                    .fill(Color32::from_rgb(17, 27, 43))
                    .inner_margin(egui::Margin::symmetric(22, 9)),
            )
            .show(ui, |ui| {
                ui.horizontal_wrapped(|ui| {
                    if self.planning {
                        ui.label(RichText::new("직접 작성하는 구현 계획").color(ACCENT));
                        ui.add(
                            egui::TextEdit::singleline(&mut self.session.plan.summary)
                                .hint_text("이 계획으로 무엇을 변경하나요?")
                                .desired_width(420.0),
                        );
                        ui.add(
                            egui::Slider::new(&mut self.zoom, 0.6..=1.3)
                                .show_value(false)
                                .text("확대"),
                        );
                    } else {
                        if ui
                            .add_enabled(!self.busy, egui::Button::new("폴더 열기"))
                            .clicked()
                        {
                            self.open_repository(ui.ctx().clone(), None);
                        }
                        if ui
                            .add_enabled(!self.busy, egui::Button::new("원격 저장소"))
                            .clicked()
                        {
                            self.connection_open = true;
                        }
                        if self.repository.is_some()
                            && ui
                                .add_enabled(!self.busy, egui::Button::new("새로고침"))
                                .clicked()
                        {
                            if self.remote {
                                self.remote_repository(ui.ctx().clone());
                            } else {
                                self.open_repository(
                                    ui.ctx().clone(),
                                    Some(PathBuf::from(&self.session.repository)),
                                );
                            }
                        }
                        if let Some(snapshot) = self.repository.clone() {
                            ui.separator();
                            ui.label(RichText::new(&snapshot.source_root).color(MUTED));
                        } else {
                            ui.label(
                                RichText::new("Layer → Slice → index 그룹 → 하위 그룹·파일")
                                    .color(MUTED),
                            );
                        }
                    }
                });
            });
    }

    fn navigate(&mut self, path: &str) {
        self.inspected_path = path.into();
        self.search.clear();
        if let Some(snapshot) = self.repository.as_ref() {
            self.repository_graph.reveal(&snapshot.tree, path);
        }
    }

    fn repository_ui(&mut self, ui: &mut egui::Ui) {
        let Some(snapshot) = self.repository.clone() else {
            ui.vertical_centered(|ui| {
                ui.add_space(100.0);
                ui.heading("지금 있는 구조에서 시작하세요");
                ui.add_space(12.0);
                ui.label("같은 의존 관계 그래프 안에서 Layer와 Slice를 펼쳐 내부 구조를 봅니다.");
                ui.add_space(18.0);
                if ui
                    .add_enabled(
                        !self.busy,
                        egui::Button::new("FSD 저장소 폴더 열기").fill(ACCENT),
                    )
                    .clicked()
                {
                    self.open_repository(ui.ctx().clone(), None);
                }
            });
            return;
        };
        if !self.inspected_path.is_empty() {
            egui::Panel::right("repository-inspector")
                .default_size(310.0)
                .min_size(250.0)
                .max_size(480.0)
                .frame(egui::Frame::new().fill(PANEL).inner_margin(18))
                .show(ui, |ui| {
                    egui::ScrollArea::vertical()
                        .id_salt("repo-details")
                        .show(ui, |ui| {
                            if let Some(entry) = snapshot.tree.find(&self.inspected_path) {
                                ui.label(
                                    RichText::new(kind_label(&entry.kind))
                                        .color(canvas::layer_color(&entry.path))
                                        .size(11.0),
                                );
                                ui.horizontal(|ui| {
                                    ui.heading(&entry.name);
                                    if ui.small_button("×").clicked() {
                                        self.inspected_path.clear();
                                    }
                                });
                                ui.label(RichText::new(&entry.path).color(MUTED));
                                ui.label(format!("파일 {}개", entry.file_count));
                                if let Some(index) = &entry.index
                                    && ui
                                        .link(
                                            RichText::new(format!("공개 진입점: {index}"))
                                                .color(ACCENT),
                                        )
                                        .clicked()
                                {
                                    self.navigate(index);
                                }
                                ui.add_space(8.0);
                                let valid_module = Plan {
                                    nodes: vec![snapshot.module(entry)],
                                    ..Default::default()
                                }
                                .errors()
                                .is_empty();
                                if ui
                                    .add_enabled(
                                        valid_module,
                                        egui::Button::new("이 그룹을 계획에 추가")
                                            .fill(Color32::from_rgb(60, 47, 102)),
                                    )
                                    .on_disabled_hover_text(
                                        "슬라이스 내부의 세그먼트·그룹·파일을 선택하세요.",
                                    )
                                    .clicked()
                                {
                                    self.add_entry(&snapshot, entry);
                                }
                                ui.separator();
                                for incoming in [false, true] {
                                    let links = snapshot.relations(entry, incoming);
                                    ui.label(
                                        RichText::new(format!(
                                            "{} · {}",
                                            if incoming {
                                                "이 그룹을 쓰는 곳"
                                            } else {
                                                "이 그룹이 쓰는 곳"
                                            },
                                            links.len()
                                        ))
                                        .strong(),
                                    );
                                    if links.is_empty() {
                                        ui.label(RichText::new("확인된 관계 없음").color(MUTED));
                                    }
                                    for (path, name) in links.iter().take(100) {
                                        if ui.link(path).clicked() {
                                            self.navigate(path);
                                        }
                                        ui.label(RichText::new(name).size(11.0).color(MUTED));
                                    }
                                    if links.len() > 100 {
                                        ui.label(format!("외 {}건", links.len() - 100));
                                    }
                                    ui.add_space(12.0);
                                }
                                egui::CollapsingHeader::new("실제 파일 목록").show(ui, |ui| {
                                    for file in entry.files() {
                                        ui.label(RichText::new(file).small().color(MUTED));
                                    }
                                });
                            }
                        });
                });
        }
        egui::CentralPanel::default()
            .frame(egui::Frame::new().fill(BACKGROUND).inner_margin(16))
            .show(ui, |ui| {
                ui.horizontal_wrapped(|ui| {
                    ui.heading("저장소 그래프");
                    if ui.button("Layer 보기").clicked() {
                        self.repository_graph.layers();
                        self.inspected_path.clear();
                    }
                    if ui.button("Slice 보기").clicked() {
                        self.repository_graph.slices(&snapshot);
                        self.inspected_path.clear();
                    }
                    ui.add(
                        egui::Slider::new(
                            &mut self.repository_graph.zoom,
                            canvas::GRAPH_ZOOM_MIN..=canvas::GRAPH_ZOOM_MAX,
                        )
                        .show_value(false)
                        .text("확대"),
                    )
                    .on_hover_text("그래프 위에서 Ctrl+휠로 확대·축소할 수 있습니다.");
                    ui.add(
                        egui::TextEdit::singleline(&mut self.search)
                            .hint_text("이름·경로 검색")
                            .desired_width(200.0),
                    );
                });
                if !self.search.is_empty() {
                    egui::CollapsingHeader::new(
                        "검색 결과 · 선택하면 그래프에서 해당 위치를 펼칩니다",
                    )
                    .default_open(true)
                    .show(ui, |ui| {
                        egui::ScrollArea::vertical()
                            .id_salt("graph-search-results")
                            .max_height(130.0)
                            .show(ui, |ui| {
                                let mut clicked = None;
                                search_tree(
                                    ui,
                                    &snapshot.tree,
                                    &self.search.to_lowercase(),
                                    &mut clicked,
                                );
                                if let Some(path) = clicked {
                                    self.navigate(&path);
                                }
                            });
                    });
                }
                self.repository_graph
                    .show(ui, &snapshot, &mut self.inspected_path);
            });
    }

    fn plan_ui(&mut self, ui: &mut egui::Ui) {
        let selected = self.selected.clone();
        if let Some(id) = selected
            .as_ref()
            .filter(|id| self.session.plan.nodes.iter().any(|node| &node.id == *id))
        {
            egui::Panel::right("module-editor")
                .default_size(360.0)
                .min_size(280.0)
                .max_size(550.0)
                .frame(egui::Frame::new().fill(PANEL).inner_margin(18))
                .show(ui, |ui| {
                    ui.horizontal(|ui| {
                        ui.heading("모듈 편집");
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            if ui.small_button("×").clicked() {
                                self.selected = None;
                            }
                        });
                    });
                    egui::ScrollArea::vertical()
                        .id_salt(("module-editor-scroll", id))
                        .show(ui, |ui| {
                            let index = self
                                .session
                                .plan
                                .nodes
                                .iter()
                                .position(|node| &node.id == id)
                                .unwrap();
                            ui.push_id(id, |ui| {
                                let node = &mut self.session.plan.nodes[index];
                                field(ui, "이름", &mut node.name, false);
                                ui.horizontal(|ui| {
                                    ui.label("변경");
                                    for (kind, label) in [
                                        ("new", "신규"),
                                        ("modify", "수정"),
                                        ("existing", "기존 참조"),
                                    ] {
                                        ui.selectable_value(&mut node.kind, kind.into(), label);
                                    }
                                });
                                field(ui, "배치 · Layer / Slice / Segment", &mut node.path, false);
                                field(ui, "책임 · 이 모듈이 하는 일", &mut node.description, true);
                                list_editor(
                                    ui,
                                    "파일 · 배치 경로 기준, 한 줄에 하나",
                                    &mut node.files,
                                );
                                ui.separator();
                                ui.horizontal(|ui| {
                                    ui.label(RichText::new("인터페이스").strong());
                                    if ui.small_button("＋ 추가").clicked() {
                                        node.interfaces.push(Interface {
                                            name: "interface".into(),
                                            ..Default::default()
                                        });
                                    }
                                });
                                let mut remove_interface = None;
                                for (i, item) in node.interfaces.iter_mut().enumerate() {
                                    ui.push_id(i, |ui| {
                                        egui::CollapsingHeader::new(format!(
                                            "{} · {}",
                                            item.name, item.kind
                                        ))
                                        .id_salt("interface")
                                        .default_open(true)
                                        .show(ui, |ui| {
                                            field(ui, "이름", &mut item.name, false);
                                            egui::ComboBox::from_id_salt("kind")
                                                .selected_text(&item.kind)
                                                .show_ui(ui, |ui| {
                                                    for kind in [
                                                        "component",
                                                        "hook",
                                                        "function",
                                                        "store",
                                                        "constant",
                                                        "type",
                                                        "server",
                                                    ] {
                                                        ui.selectable_value(
                                                            &mut item.kind,
                                                            kind.into(),
                                                            kind,
                                                        );
                                                    }
                                                });
                                            field(ui, "조건 · 언제", &mut item.condition, true);
                                            field(
                                                ui,
                                                "결과 · 무엇을 보장하나",
                                                &mut item.result,
                                                true,
                                            );
                                            list_editor(
                                                ui,
                                                "입력 · 한 줄에 하나",
                                                &mut item.inputs,
                                            );
                                            list_editor(
                                                ui,
                                                "출력 · 한 줄에 하나",
                                                &mut item.outputs,
                                            );
                                            if ui.small_button("인터페이스 제거").clicked() {
                                                remove_interface = Some(i);
                                            }
                                        });
                                    });
                                }
                                if let Some(i) = remove_interface {
                                    node.interfaces.remove(i);
                                }
                                ui.separator();
                                ui.label(RichText::new("의존 관계").strong());
                                let targets: Vec<_> = self
                                    .session
                                    .plan
                                    .nodes
                                    .iter()
                                    .filter(|node| &node.id != id)
                                    .map(|node| {
                                        (node.id.clone(), format!("{} · {}", node.name, node.path))
                                    })
                                    .collect();
                                let selected_name = targets
                                    .iter()
                                    .find(|(target, _)| target == &self.connect_to)
                                    .map(|(_, name)| name.as_str())
                                    .unwrap_or("대상 모듈 선택");
                                egui::ComboBox::from_id_salt("connect-to")
                                    .selected_text(selected_name)
                                    .width(ui.available_width())
                                    .show_ui(ui, |ui| {
                                        for (target, name) in &targets {
                                            ui.selectable_value(
                                                &mut self.connect_to,
                                                target.clone(),
                                                name,
                                            );
                                        }
                                    });
                                let valid_target =
                                    targets.iter().any(|(target, _)| target == &self.connect_to);
                                if ui
                                    .add_enabled(
                                        valid_target,
                                        egui::Button::new("이 모듈 → 대상 모듈 연결"),
                                    )
                                    .clicked()
                                    && !self
                                        .session
                                        .plan
                                        .edges
                                        .iter()
                                        .any(|edge| edge.from == *id && edge.to == self.connect_to)
                                {
                                    self.session.plan.edges.push(Edge {
                                        from: id.clone(),
                                        to: self.connect_to.clone(),
                                        ..Default::default()
                                    });
                                }
                                let mut remove_edge = None;
                                for edge in self
                                    .session
                                    .plan
                                    .edges
                                    .iter_mut()
                                    .filter(|edge| edge.from == *id || edge.to == *id)
                                {
                                    ui.push_id(&edge.id, |ui| {
                                        let outgoing = edge.from == *id;
                                        let other = if outgoing { &edge.to } else { &edge.from };
                                        let name = self
                                            .session
                                            .plan
                                            .nodes
                                            .iter()
                                            .find(|node| &node.id == other)
                                            .map(|node| node.name.as_str())
                                            .unwrap_or("?");
                                        ui.label(format!(
                                            "{} {name}",
                                            if outgoing { "→" } else { "←" }
                                        ));
                                        egui::ComboBox::from_id_salt("edge-kind")
                                            .selected_text(&edge.kind)
                                            .show_ui(ui, |ui| {
                                                for kind in [
                                                    "uses",
                                                    "imports",
                                                    "reads",
                                                    "emits",
                                                    "navigates",
                                                ] {
                                                    ui.selectable_value(
                                                        &mut edge.kind,
                                                        kind.into(),
                                                        kind,
                                                    );
                                                }
                                            });
                                        list_editor(
                                            ui,
                                            "사용하는 인터페이스 · 한 줄에 하나",
                                            &mut edge.uses,
                                        );
                                        if ui.small_button("연결 해제").clicked() {
                                            remove_edge = Some(edge.id.clone());
                                        }
                                    });
                                    ui.separator();
                                }
                                if let Some(edge) = remove_edge {
                                    self.session.plan.edges.retain(|item| item.id != edge);
                                }
                                ui.add_space(15.0);
                                if ui
                                    .button(
                                        RichText::new("모듈 제거")
                                            .color(Color32::from_rgb(243, 169, 171)),
                                    )
                                    .clicked()
                                {
                                    self.session.plan.nodes.remove(index);
                                    self.session
                                        .plan
                                        .edges
                                        .retain(|edge| edge.from != *id && edge.to != *id);
                                    self.selected = None;
                                }
                            });
                        });
                });
        }
        egui::CentralPanel::default().frame(egui::Frame::new().fill(BACKGROUND).inner_margin(10)).show(ui, |ui| {
            if self.session.plan.nodes.is_empty() {
                ui.vertical_centered(|ui| {
                    ui.add_space(120.0); ui.heading("설계는 직접, 계획은 명확하게");
                    ui.add_space(12.0); ui.label("저장소에서 기존 그룹을 가져오거나 새 모듈을 추가하세요.");
                    ui.label(RichText::new("책임 · 파일 · 인터페이스 · 의존 관계를 작성한 뒤 JSON 또는 Markdown으로 내보냅니다.").color(MUTED));
                    ui.add_space(18.0);
                    if ui.button("＋ 첫 모듈 추가").clicked() { self.add_module(); }
                });
            } else { canvas::plan(ui, &self.session.plan, &mut self.selected, self.zoom); }
        });
    }

    fn connection(&mut self, ctx: &egui::Context) {
        if !self.connection_open {
            return;
        }
        let mut open = true;
        egui::Window::new("원격 저장소")
            .open(&mut open)
            .collapsible(false)
            .resizable(false)
            .default_width(450.0)
            .show(ctx, |ui| {
                ui.label("현재 서버 체크아웃의 폴더와 import를 읽습니다.");
                field(ui, "서버 주소 · HTTPS", &mut self.session.server, false);
                ui.label(RichText::new("Factory API 토큰").small().color(MUTED));
                ui.add(
                    egui::TextEdit::singleline(&mut self.token)
                        .password(true)
                        .desired_width(f32::INFINITY),
                );
                ui.checkbox(
                    &mut self.remember_token,
                    "연결 성공 후 OS 자격 증명 저장소에 토큰 기억",
                );
                if ui.button("기존 앱에서 저장한 토큰 읽기").clicked() {
                    match transport::token(true, "") {
                        Ok(token) => self.token = token,
                        Err(error) => self.notify(error, true),
                    }
                }
                ui.add_space(10.0);
                if ui
                    .add_enabled(!self.busy, egui::Button::new("저장소 구조 불러오기"))
                    .clicked()
                {
                    self.remote_repository(ctx.clone());
                }
                if self.error {
                    ui.colored_label(Color32::LIGHT_RED, &self.message);
                }
            });
        if !open {
            self.connection_open = false;
        }
    }

    fn persist(&mut self) {
        if self.storage_enabled && self.dirty {
            match storage::save(&self.session) {
                Ok(()) => self.dirty = false,
                Err(error) => {
                    self.storage_enabled = false;
                    self.notify(
                        format!("자동 저장 실패: {error}. JSON으로 내보내세요."),
                        true,
                    );
                }
            }
        }
    }
}

impl eframe::App for Workbench {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        self.receive();
        self.history_action = false;
        let before = self.session.plan.clone();
        if ui.input_mut(|input| input.consume_key(egui::Modifiers::COMMAND, egui::Key::S)) {
            self.export(ui.ctx().clone(), false);
        }
        if !ui.ctx().egui_wants_keyboard_input() {
            if ui.input_mut(|input| input.consume_key(egui::Modifiers::COMMAND, egui::Key::Z)) {
                self.undo(false);
            }
            if ui.input_mut(|input| {
                input.consume_key(
                    egui::Modifiers::COMMAND.plus(egui::Modifiers::SHIFT),
                    egui::Key::Z,
                )
            }) {
                self.undo(true);
            }
        }
        self.header(ui);
        egui::Panel::bottom("status")
            .frame(
                egui::Frame::new()
                    .fill(PANEL)
                    .inner_margin(egui::Margin::symmetric(22, 9)),
            )
            .show(ui, |ui| {
                ui.horizontal_wrapped(|ui| {
                    if self.busy {
                        ui.spinner();
                    }
                    ui.label(
                        RichText::new(&self.message)
                            .size(11.0)
                            .color(if self.error {
                                Color32::LIGHT_RED
                            } else {
                                MUTED
                            }),
                    );
                    if self.planning {
                        let errors = self.session.plan.errors();
                        if !errors.is_empty() {
                            ui.label(
                                RichText::new(format!("확인 {}건", errors.len()))
                                    .color(Color32::LIGHT_RED),
                            )
                            .on_hover_text(errors.join("\n"));
                        }
                    }
                });
            });
        if self.planning {
            self.plan_ui(ui);
        } else {
            self.repository_ui(ui);
        }
        self.connection(ui.ctx());
        if before != self.session.plan && !self.history_action {
            if !self.dirty || self.last_edit.elapsed() > Duration::from_millis(700) {
                self.history.push(before);
                if self.history.len() > 40 {
                    self.history.remove(0);
                }
            }
            self.redo.clear();
            self.session.plan.version = self.session.plan.version.saturating_add(1);
            self.last_edit = Instant::now();
            self.dirty = true;
        }
        if self.dirty {
            if self.last_edit.elapsed() >= Duration::from_millis(700) {
                self.persist();
            } else {
                ui.ctx().request_repaint_after(Duration::from_millis(700));
            }
        }
    }
    fn on_exit(&mut self, _gl: Option<&eframe::glow::Context>) {
        self.persist();
    }
}

fn field(ui: &mut egui::Ui, label: &str, text: &mut String, multiline: bool) {
    ui.label(RichText::new(label).size(11.0).color(MUTED));
    let widget = if multiline {
        egui::TextEdit::multiline(text).desired_rows(2)
    } else {
        egui::TextEdit::singleline(text)
    };
    ui.add(widget.desired_width(f32::INFINITY));
}

fn list_editor(ui: &mut egui::Ui, label: &str, values: &mut Vec<String>) {
    let mut text = values.join("\n");
    ui.label(RichText::new(label).size(11.0).color(MUTED));
    let response = ui.add(
        egui::TextEdit::multiline(&mut text)
            .desired_rows(2)
            .desired_width(f32::INFINITY),
    );
    if response.changed() {
        *values = text.split('\n').map(str::to_owned).collect();
    }
    if response.lost_focus() {
        values.retain(|value| !value.trim().is_empty());
    }
}

fn kind_label(kind: &str) -> &str {
    match kind {
        "layer" => "LAYER",
        "slice" => "SLICE",
        "group" => "INDEX GROUP",
        "file" => "FILE",
        _ => "FOLDER",
    }
}

fn search_tree(ui: &mut egui::Ui, entry: &Entry, query: &str, clicked: &mut Option<String>) {
    if !entry.path.is_empty()
        && entry.path.to_lowercase().contains(query)
        && ui.link(&entry.path).clicked()
    {
        *clicked = Some(entry.path.clone());
    }
    for child in &entry.children {
        search_tree(ui, child, query, clicked);
    }
}
