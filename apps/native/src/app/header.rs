use super::*;
impl Workbench {
    pub(super) fn header(&mut self, ui: &mut egui::Ui) {
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
}
