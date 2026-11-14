use super::viewport::Transform;
use super::*;

pub(super) fn background(
    ui: &egui::Ui,
    scene: &Scene,
    canvas: Rect,
    selected: &str,
    transform: Transform,
) {
    let scale = transform.scale;
    let origin = transform.origin;
    let painter = ui.painter();
    let visible = canvas.intersect(ui.clip_rect());
    let step = 24.0 * scale;
    let start = origin
        + egui::vec2(
            ((visible.left() - canvas.left()) / step).floor() * step,
            ((visible.top() - canvas.top()) / step).floor() * step,
        );
    for x in 0..(visible.width() / step) as i32 + 2 {
        for y in 0..(visible.height() / step) as i32 + 2 {
            painter.circle_filled(
                start + egui::vec2(x as f32 * step, y as f32 * step),
                0.7,
                BORDER.gamma_multiply(0.5),
            );
        }
    }
    // Keep all expanded boundaries behind both the child nodes and their import lines.
    for item in scene.boxes.iter().filter(|item| item.expanded) {
        let rect = transform.rect(item.rect);
        if !ui.is_rect_visible(rect) {
            continue;
        }
        let color = canvas::layer_color(&item.path);
        let mut painter = painter.clone();
        if item.dimmed {
            painter.multiply_opacity(0.24);
        }
        let depth = item.depth.min(5) as u8;
        painter.rect_filled(
            rect,
            12.0 * scale,
            Color32::from_rgb(13 + depth * 3, 21 + depth * 3, 34 + depth * 4),
        );
        painter.rect_stroke(
            rect,
            12.0 * scale,
            Stroke::new(
                1.0,
                if item.path == selected {
                    ACCENT
                } else {
                    color.gamma_multiply(0.45)
                },
            ),
            StrokeKind::Inside,
        );
    }
}

pub(super) fn edges(ui: &egui::Ui, scene: &Scene, selected: &str, transform: Transform) {
    let scale = transform.scale;
    let painter = ui.painter();
    for route in &scene.edges {
        let Some(relation) = relation(&route.edge, selected) else {
            continue;
        };
        if route.points.len() < 2 {
            continue;
        }
        let color = if relation == Direction::Incoming {
            Color32::from_rgb(100, 207, 189)
        } else {
            ACCENT
        };
        let points: Vec<_> = route
            .points
            .iter()
            .copied()
            .map(|p| transform.point(p))
            .collect();
        // A dark gap at a crossing distinguishes an overpass from a junction.
        painter.add(egui::Shape::line(
            points.clone(),
            Stroke::new(6.0, BACKGROUND),
        ));
        painter.add(egui::Shape::line(points.clone(), Stroke::new(2.0, color)));
        if let Some((end, previous)) = points.last().zip(points.iter().rev().nth(1)) {
            let direction = (*end - *previous).normalized();
            let perpendicular = egui::vec2(-direction.y, direction.x);
            painter.add(egui::Shape::convex_polygon(
                vec![
                    *end,
                    *end - direction * 8.0 * scale + perpendicular * 4.0 * scale,
                    *end - direction * 8.0 * scale - perpendicular * 4.0 * scale,
                ],
                color,
                Stroke::NONE,
            ));
        }
    }
}

pub(super) fn labels(
    ui: &mut egui::Ui,
    scene: &Scene,
    canvas: Rect,
    selected: &str,
    transform: Transform,
) {
    let scale = transform.scale;
    let visible = canvas.intersect(ui.clip_rect());
    // Import details remain on the graph itself rather than requiring the side inspector.
    for (i, route) in scene.edges.iter().enumerate() {
        if !scene.shows(&route.edge, selected) {
            continue;
        }
        let Some(anchor) = route.label_position() else {
            continue;
        };
        let anchor = transform.point(anchor);
        let label = format!("{}", route.edge.file_pairs);
        let label_rect = Rect::from_center_size(anchor, egui::vec2(24.0, 18.0) * scale);
        if !ui.is_rect_visible(label_rect) {
            continue;
        }
        let painter = ui.painter();
        painter.rect_filled(label_rect, 4.0, BACKGROUND);
        painter.text(
            anchor,
            Align2::CENTER_CENTER,
            label,
            FontId::proportional(10.0 * scale),
            MUTED,
        );
        ui.interact(
            label_rect,
            ui.scope_id().with(("import-count", i)),
            Sense::hover(),
        )
        .on_hover_text(format!(
            "{} → {}\n파일 관계 {}개\n{}",
            route.edge.from,
            route.edge.to,
            route.edge.file_pairs,
            route.edge.names.join(", ")
        ));
    }
    if let Some(pointer) = ui
        .input(|input| input.pointer.hover_pos())
        .filter(|pointer| visible.contains(*pointer))
    {
        let closest = scene
            .edges
            .iter()
            .enumerate()
            .filter(|(_, route)| scene.shows(&route.edge, selected))
            .filter_map(|(i, route)| {
                let distance = route
                    .points
                    .windows(2)
                    .map(|segment| {
                        let a = transform.point(segment[0]);
                        let b = transform.point(segment[1]);
                        let vector = b - a;
                        let t = ((pointer - a).dot(vector) / vector.length_sq().max(0.01))
                            .clamp(0.0, 1.0);
                        (pointer - (a + vector * t)).length_sq()
                    })
                    .min_by(f32::total_cmp)?;
                Some((i, route, distance))
            })
            .min_by(|a, b| a.2.total_cmp(&b.2));
        if let Some((i, route, _)) = closest.filter(|(_, _, distance)| *distance < 25.0) {
            ui.interact(
                Rect::from_center_size(pointer, Vec2::splat(8.0)),
                ui.scope_id().with(("import-line", i)),
                Sense::hover(),
            )
            .on_hover_text(format!(
                "{} → {}\n파일 관계 {}개\n{}",
                route.edge.from,
                route.edge.to,
                route.edge.file_pairs,
                route.edge.names.join(", ")
            ));
        }
    }
}
