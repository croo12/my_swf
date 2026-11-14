mod connection;
mod fields;
mod files;
mod header;
mod history;
mod plan;
mod repository;
#[cfg(test)]
mod tests;

use crate::{
    canvas::{self, ACCENT, BACKGROUND, BORDER, MUTED, PANEL, TEXT},
    file_access::FileAccess,
    repository_graph,
};
use eframe::egui::{self, Color32, RichText};
use factory_plan::Plan;
use factory_repository::Snapshot;
use factory_session::Session;
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
    files: FileAccess,
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
        Self::with_file_access(cc, initial, FileAccess::native())
    }

    fn with_file_access(
        cc: &eframe::CreationContext<'_>,
        initial: Option<PathBuf>,
        files: FileAccess,
    ) -> Self {
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
        let (session, message, storage_enabled) = match files.load() {
            Ok(session) => (
                session,
                "저장소를 열어 구조를 살펴보거나 직접 계획을 작성하세요.".into(),
                true,
            ),
            Err(error) => (Session::default(), error, false),
        };
        let (sender, receiver) = mpsc::channel();
        let mut app = Self {
            files,
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

    fn persist(&mut self) {
        if self.storage_enabled && self.dirty {
            match self.files.save(&self.session) {
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
