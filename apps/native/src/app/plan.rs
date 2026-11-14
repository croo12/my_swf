use super::fields::{field, list_editor};
use super::*;
use factory_plan::{Edge, Interface};
impl Workbench {
    pub(super) fn plan_ui(&mut self, ui: &mut egui::Ui) {
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
                                                })
                                                .response
                                                .accessible_name("인터페이스 종류");
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
                                    })
                                    .response
                                    .accessible_name("의존 대상");
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
                                            })
                                            .response
                                            .accessible_name("관계 종류");
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
}
