use super::card::{Card, card};
use super::*;

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
