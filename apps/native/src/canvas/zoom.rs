use super::*;

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
