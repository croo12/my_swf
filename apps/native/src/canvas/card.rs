use super::*;

pub struct Card<'a> {
    pub title: &'a str,
    pub path: &'a str,
    pub badge: &'a str,
    pub description: &'a str,
    pub footer: &'a str,
    pub selected: bool,
}

pub fn card(
    ui: &mut egui::Ui,
    rect: Rect,
    id: egui::Id,
    card: Card<'_>,
    scale: f32,
) -> egui::Response {
    let response = ui.interact(rect, id, Sense::click());
    response.widget_info(|| {
        egui::WidgetInfo::labeled(
            egui::Role::Button,
            ui.is_enabled(),
            format!("{} 선택", card.path),
        )
    });
    if !ui.is_rect_visible(rect) {
        return response;
    }
    let painter = ui.painter().with_clip_rect(rect.intersect(ui.clip_rect()));
    let color = layer_color(card.path);
    let fill = if card.selected {
        Color32::from_rgb(38, 39, 67)
    } else if response.hovered() {
        Color32::from_rgb(30, 43, 65)
    } else {
        CARD
    };
    painter.rect_filled(rect, 11.0 * scale, fill);
    painter.rect_stroke(
        rect,
        11.0 * scale,
        Stroke::new(
            if card.selected { 1.5 } else { 1.0 },
            if card.selected { ACCENT } else { BORDER },
        ),
        StrokeKind::Inside,
    );
    painter.rect_filled(
        Rect::from_min_size(
            rect.min + egui::vec2(1.0, 13.0) * scale,
            egui::vec2(3.0, rect.height() / scale - 26.0) * scale,
        ),
        2.0,
        color,
    );
    let start = rect.min + egui::vec2(17.0, 14.0) * scale;
    let width = rect.width() - 32.0 * scale;
    let text = |position: Pos2, value: &str, size: f32, color: Color32, max_height: f32| {
        let mut job = egui::text::LayoutJob::simple(
            value.to_owned(),
            FontId::proportional(size * scale),
            color,
            width,
        );
        job.wrap.max_rows = (max_height / size).max(1.0) as usize;
        job.wrap.break_anywhere = true;
        let galley = painter.layout_job(job);
        painter.galley(position, galley, color);
    };
    text(start, card.badge, 11.0, color, 12.0);
    text(
        start + egui::vec2(0.0, 24.0) * scale,
        card.title,
        17.0,
        TEXT,
        20.0,
    );
    text(
        start + egui::vec2(0.0, 51.0) * scale,
        card.path,
        11.0,
        MUTED,
        13.0,
    );
    text(
        start + egui::vec2(0.0, 75.0) * scale,
        card.description,
        12.0,
        TEXT,
        31.0,
    );
    painter.line_segment(
        [
            rect.left_bottom() + egui::vec2(14.0, -31.0) * scale,
            rect.right_bottom() + egui::vec2(-14.0, -31.0) * scale,
        ],
        Stroke::new(1.0, BORDER),
    );
    text(
        rect.left_bottom() + egui::vec2(17.0, -23.0) * scale,
        card.footer,
        11.0,
        MUTED,
        13.0,
    );
    response.on_hover_text(card.path)
}
