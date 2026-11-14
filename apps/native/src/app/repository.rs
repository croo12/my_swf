use super::*;
use factory_repository::Entry;
impl Workbench {
    pub(super) fn navigate(&mut self, path: &str) {
        self.inspected_path = path.into();
        self.search.clear();
        if let Some(snapshot) = self.repository.as_ref() {
            self.repository_graph.reveal(&snapshot.tree, path);
        }
    }

    pub(super) fn repository_ui(&mut self, ui: &mut egui::Ui) {
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
                                    nodes: vec![factory_plan::Node::from_repository(
                                        &snapshot, entry,
                                    )],
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
                    )
                    .accessible_name("저장소 검색");
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
