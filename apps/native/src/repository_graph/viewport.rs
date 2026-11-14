use super::*;

#[derive(Clone, Copy)]
pub(super) struct Transform {
    pub origin: Pos2,
    pub scale: f32,
}
impl Transform {
    pub fn point(self, point: Pos2) -> Pos2 {
        self.origin + point.to_vec2() * self.scale
    }
    pub fn rect(self, rect: Rect) -> Rect {
        Rect::from_min_max(self.point(rect.min), self.point(rect.max))
    }
}

impl View {
    pub(super) fn viewport(&mut self, ui: &mut egui::Ui, selected: &mut String) {
        let scene = self.scene.as_ref().unwrap();
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
            let transform = Transform {
                origin: canvas.min,
                scale,
            };
            super::paint::background(ui, scene, canvas, selected, transform);
            super::paint::edges(ui, scene, selected, transform);
            let action = super::cards::draw(ui, scene, selected, requested.as_deref(), transform);
            clicked = action.clicked;
            toggle = action.toggle;
            super::paint::labels(ui, scene, canvas, selected, transform);
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
