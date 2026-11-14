use eframe::egui::{self, Align2, Color32, FontId, Pos2, Rect, Sense, Stroke, StrokeKind, Vec2};
use factory_core::{plan::Plan, repository::LAYERS};
use std::collections::HashMap;

pub const BACKGROUND: Color32 = Color32::from_rgb(10, 16, 27);
pub const PANEL: Color32 = Color32::from_rgb(15, 23, 37);
pub const CARD: Color32 = Color32::from_rgb(22, 33, 51);
pub const BORDER: Color32 = Color32::from_rgb(51, 66, 90);
pub const TEXT: Color32 = Color32::from_rgb(231, 237, 247);
pub const MUTED: Color32 = Color32::from_rgb(145, 161, 187);
pub const ACCENT: Color32 = Color32::from_rgb(155, 138, 251);
pub const GRAPH_ZOOM_MIN: f32 = 0.08;
pub const GRAPH_ZOOM_MAX: f32 = 1.3;

/// Change graph scale around the world point under the pointer, leaving the app UI alone.
pub fn zoom_at_pointer(ui: &egui::Ui, zoom: &mut f32, scroll_id: &str) -> Option<Vec2> {
    let viewport = ui.available_rect_before_wrap().intersect(ui.clip_rect());
    if !ui.rect_contains_pointer(viewport) {
        return None;
    }
    let (pointer, factor) = ui.input(|input| (input.pointer.hover_pos(), input.zoom_delta()));
    let pointer = pointer?;
    if factor == 1.0 || !factor.is_finite() || factor <= 0.0 {
        return None;
    }
    let previous = *zoom;
    *zoom = (previous * factor).clamp(GRAPH_ZOOM_MIN, GRAPH_ZOOM_MAX);
    let offset = egui::scroll_area::State::load(
        ui.ctx(),
        ui.make_persistent_id(egui::IdSalt::new(scroll_id)),
    )
    .map(|state| state.unclamped_offset())
    .unwrap_or_default();
    let anchor = pointer - viewport.min;
    let offset = ((offset + anchor) * (*zoom / previous) - anchor).max(Vec2::ZERO);
    ui.ctx().request_repaint();
    Some(offset)
}

#[cfg(test)]
mod zoom_tests {
    use super::*;

    fn frame(
        ctx: &egui::Context,
        zoom: &mut f32,
        pointer: Pos2,
        wheel: Option<(egui::Modifiers, f32)>,
        initial: bool,
    ) -> (Vec2, Pos2) {
        let mut events = vec![egui::Event::PointerMoved(pointer)];
        if let Some((modifiers, delta)) = wheel {
            events.push(egui::Event::ModifiersChanged(modifiers));
            events.push(egui::Event::MouseWheel {
                unit: egui::MouseWheelUnit::Line,
                delta: egui::vec2(0.0, delta),
                phase: egui::TouchPhase::Move,
                source: egui::MouseWheelSource::Wheel,
                modifiers,
            });
        }
        let input = egui::RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, egui::vec2(800.0, 600.0))),
            events,
            ..Default::default()
        };
        let mut observed = None;
        let mut output = ctx.run_ui(input, |ui| {
            egui::Panel::right("inspector")
                .default_size(200.0)
                .resizable(false)
                .show(ui, |ui| {
                    ui.label("Inspector");
                });
            egui::CentralPanel::default().show(ui, |ui| {
                let offset = zoom_at_pointer(ui, zoom, "graph-zoom-test");
                let mut scroll = egui::ScrollArea::both()
                    .id_salt("graph-zoom-test")
                    .auto_shrink([false, false]);
                if initial {
                    scroll = scroll.scroll_offset(egui::vec2(200.0, 300.0));
                } else if let Some(offset) = offset {
                    scroll = scroll.scroll_offset(offset);
                }
                let output = scroll.show(ui, |ui| {
                    ui.allocate_exact_size(egui::vec2(8000.0, 6000.0) * *zoom, Sense::hover());
                });
                let offset = output.state.unclamped_offset();
                let world = ((pointer - output.inner_rect.min + offset) / *zoom).to_pos2();
                observed = Some((offset, world));
            });
        });
        output.textures_delta.clear(); // This headless interaction test has no renderer.
        observed.unwrap()
    }

    #[test]
    fn ctrl_wheel_zooms_at_pointer_and_does_not_zoom_over_inspector_or_normal_scroll() {
        for (modifiers, delta, pointer, expected_direction) in [
            (egui::Modifiers::CTRL, 2.0, egui::pos2(300.0, 250.0), 1),
            (egui::Modifiers::CTRL, -2.0, egui::pos2(300.0, 250.0), -1),
            (egui::Modifiers::CTRL, 2.0, egui::pos2(750.0, 250.0), 0),
            (egui::Modifiers::NONE, 2.0, egui::pos2(300.0, 250.0), 0),
        ] {
            let ctx = egui::Context::default();
            ctx.options_mut(|options| {
                options.input_options.zoom_modifier = egui::Modifiers {
                    ctrl: true,
                    command: true,
                    ..Default::default()
                };
            });
            let mut zoom = 0.5;
            let (before_offset, before_world) = frame(&ctx, &mut zoom, pointer, None, true);
            let (after_offset, after_world) =
                frame(&ctx, &mut zoom, pointer, Some((modifiers, delta)), false);
            match expected_direction {
                1 => assert!(zoom > 0.5),
                -1 => assert!(zoom < 0.5),
                _ => assert_eq!(zoom, 0.5),
            }
            if expected_direction != 0 {
                assert!(
                    before_world.distance(after_world) < 0.1,
                    "World anchor moved: {before_world:?} → {after_world:?}"
                );
            } else if modifiers == egui::Modifiers::NONE {
                assert_ne!(before_offset, after_offset); // The ordinary wheel still pans the graph.
            }
        }
    }
}

pub fn layer_color(path: &str) -> Color32 {
    match path.split('/').next().unwrap_or_default() {
        "app" => Color32::from_rgb(183, 149, 255),
        "pages" => Color32::from_rgb(105, 172, 255),
        "widgets" => Color32::from_rgb(92, 205, 224),
        "features" => Color32::from_rgb(112, 214, 169),
        "entities" => Color32::from_rgb(233, 190, 112),
        _ => Color32::from_rgb(183, 195, 215),
    }
}

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

/// Stable, bounded layout: same layer lanes and module cards as the previous UI.
pub fn plan(ui: &mut egui::Ui, plan: &Plan, selected: &mut Option<String>, scale: f32) {
    let card_size = egui::vec2(265.0, 166.0) * scale;
    let gap = 34.0 * scale;
    let label_width = 145.0 * scale;
    let columns = ((ui.available_width() - label_width) / (card_size.x + gap))
        .floor()
        .max(1.0) as usize;
    let mut positions = HashMap::new();
    let mut lanes = vec![];
    let mut y = 20.0 * scale;
    for layer in LAYERS {
        let nodes: Vec<_> = plan
            .nodes
            .iter()
            .filter(|node| node.path.split('/').next() == Some(layer))
            .collect();
        if nodes.is_empty() {
            continue;
        }
        let height = nodes.len().div_ceil(columns) as f32 * (card_size.y + gap) + 18.0 * scale;
        for (i, node) in nodes.iter().enumerate() {
            positions.insert(
                node.id.clone(),
                egui::pos2(
                    label_width + (i % columns) as f32 * (card_size.x + gap),
                    y + 17.0 * scale + (i / columns) as f32 * (card_size.y + gap),
                ),
            );
        }
        lanes.push((layer, y, height, nodes.len()));
        y += height + 25.0 * scale;
    }
    egui::ScrollArea::both()
        .id_salt("plan-canvas")
        .show(ui, |ui| {
            let (canvas, _) = ui.allocate_exact_size(
                Vec2::new(
                    ui.available_width().max(label_width + card_size.x + gap),
                    y.max(ui.available_height()),
                ),
                Sense::hover(),
            );
            let painter = ui.painter();
            // Grid dots are clipped to the viewport, so large plans don't paint offscreen work.
            let visible = canvas.intersect(ui.clip_rect());
            let step = 24.0 * scale;
            let start = canvas.min
                + egui::vec2(
                    ((visible.left() - canvas.left()) / step).floor() * step,
                    ((visible.top() - canvas.top()) / step).floor() * step,
                );
            for x in 0..((visible.width() / step) as i32 + 2) {
                for y in 0..((visible.height() / step) as i32 + 2) {
                    painter.circle_filled(
                        start + egui::vec2(x as f32 * step, y as f32 * step),
                        0.7,
                        BORDER.gamma_multiply(0.6),
                    );
                }
            }
            for (layer, y, height, count) in lanes {
                let rect = Rect::from_min_size(
                    canvas.min + egui::vec2(4.0, y),
                    egui::vec2(canvas.width() - 8.0, height),
                );
                painter.rect_stroke(
                    rect,
                    12.0,
                    Stroke::new(1.0, BORDER.gamma_multiply(0.65)),
                    StrokeKind::Inside,
                );
                painter.text(
                    rect.min + egui::vec2(15.0, 22.0) * scale,
                    Align2::LEFT_TOP,
                    layer.to_uppercase(),
                    FontId::proportional(13.0 * scale),
                    layer_color(layer),
                );
                painter.text(
                    rect.min + egui::vec2(15.0, 46.0) * scale,
                    Align2::LEFT_TOP,
                    format!("모듈 {count}"),
                    FontId::proportional(11.0 * scale),
                    MUTED,
                );
            }
            for edge in &plan.edges {
                let (Some(from), Some(to)) = (positions.get(&edge.from), positions.get(&edge.to))
                else {
                    continue;
                };
                let a = canvas.min + from.to_vec2() + egui::vec2(card_size.x * 0.5, card_size.y);
                let b = canvas.min + to.to_vec2() + egui::vec2(card_size.x * 0.5, 0.0);
                let active = selected
                    .as_ref()
                    .is_some_and(|id| id == &edge.from || id == &edge.to);
                let mid = (a.y + b.y) * 0.5;
                let color = if active {
                    ACCENT
                } else {
                    Color32::from_rgb(74, 92, 121)
                };
                painter.add(egui::Shape::line(
                    vec![a, egui::pos2(a.x, mid), egui::pos2(b.x, mid), b],
                    Stroke::new(if active { 2.0 } else { 1.2 }, color),
                ));
                let direction = if b.y >= a.y { 1.0 } else { -1.0 };
                painter.add(egui::Shape::convex_polygon(
                    vec![
                        b,
                        b + egui::vec2(-4.0, -7.0 * direction),
                        b + egui::vec2(4.0, -7.0 * direction),
                    ],
                    color,
                    Stroke::NONE,
                ));
            }
            for node in &plan.nodes {
                let Some(position) = positions.get(&node.id) else {
                    continue;
                };
                let rect = Rect::from_min_size(canvas.min + position.to_vec2(), card_size);
                let badge = match node.kind.as_str() {
                    "modify" => "수정",
                    "existing" => "기존",
                    _ => "신규",
                };
                let interfaces = node
                    .interfaces
                    .iter()
                    .take(3)
                    .map(|item| item.name.as_str())
                    .collect::<Vec<_>>()
                    .join(" · ");
                let footer = format!(
                    "인터페이스 {}  ·  파일 {}",
                    node.interfaces.len(),
                    node.files.len()
                );
                let description = if node.description.is_empty() {
                    interfaces.as_str()
                } else {
                    node.description.as_str()
                };
                if card(
                    ui,
                    rect,
                    ui.scope_id().with(&node.id),
                    Card {
                        title: &node.name,
                        path: &node.path,
                        badge,
                        description,
                        footer: &footer,
                        selected: selected.as_ref() == Some(&node.id),
                    },
                    scale,
                )
                .clicked()
                {
                    *selected = Some(node.id.clone());
                }
            }
        });
}
