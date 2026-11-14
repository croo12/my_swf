use super::viewport::Transform;
use super::*;

pub(super) struct Action {
    pub clicked: Option<String>,
    pub toggle: Option<String>,
}

pub(super) fn draw(
    ui: &mut egui::Ui,
    scene: &Scene,
    selected: &str,
    requested: Option<&str>,
    transform: Transform,
) -> Action {
    let scale = transform.scale;
    let mut toggle = None;
    let mut clicked = None;
    for item in &scene.boxes {
        let rect = transform.rect(item.rect);
        let header = transform.rect(item.header());
        if requested == Some(item.path.as_str()) {
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
        response
            .widget_info(|| egui::WidgetInfo::labeled(egui::Role::Button, ui.is_enabled(), &label));
        let mut painter = ui.painter().with_clip_rect(rect.intersect(ui.clip_rect()));
        if item.dimmed {
            painter.multiply_opacity(0.24);
        }
        let color = canvas::layer_color(&item.path);
        if !item.expanded {
            painter.rect_filled(
                rect,
                10.0 * scale,
                if item.path == selected {
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
                    if item.path == selected { 1.6 } else { 1.0 },
                    if item.path == selected {
                        ACCENT
                    } else {
                        BORDER
                    },
                ),
                StrokeKind::Inside,
            );
        } else if response.hovered() {
            painter.rect_filled(header.shrink(2.0), 8.0 * scale, color.gamma_multiply(0.09));
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
                egui::Button::new(egui::RichText::new(marker).size(15.0 * scale).color(color))
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
    Action { clicked, toggle }
}
