use super::*;

pub(super) fn controls(ui: &mut egui::Ui, selected: &mut String, pending: bool) -> bool {
    let mut fit = false;
    ui.horizontal_wrapped(|ui| {
        fit = ui.button("화면 맞춤").clicked();
        if ui
            .add_enabled(!selected.is_empty(), egui::Button::new("선택 해제"))
            .clicked()
        {
            selected.clear();
        }
        if pending {
            ui.spinner();
        }
    });
    fit
}

pub(super) fn summary(ui: &mut egui::Ui, scene: &Scene, selected: &str, elapsed: Duration) {
    ui.horizontal_wrapped(|ui| {
        ui.label(
            egui::RichText::new(format!(
                "ELK Libavoid · {:.0} ms",
                elapsed.as_secs_f64() * 1000.0
            ))
            .color(MUTED)
            .small(),
        );
        let shown = scene
            .edges
            .iter()
            .filter(|route| scene.shows(&route.edge, selected))
            .count();
        ui.label(
            egui::RichText::new(format!(
                "노드 {} · 선택 연결 {} / {}",
                scene.boxes.len(),
                shown,
                scene.edges.len()
            ))
            .color(MUTED)
            .small(),
        );
        ui.label(
            egui::RichText::new(
                "블록 클릭: 연결 보기 · ▸ / ▾: 펼치기 / 접기 · 드래그: 이동 · Ctrl+휠: 확대 / 축소",
            )
            .color(MUTED)
            .small(),
        );
        if !selected.is_empty() {
            ui.label(egui::RichText::new("→ 사용").color(ACCENT).small());
            ui.label(
                egui::RichText::new("← 사용됨")
                    .color(Color32::from_rgb(100, 207, 189))
                    .small(),
            );
        }
    });
    if selected.is_empty() {
        ui.label(
            egui::RichText::new(
                "블록을 클릭하면 그 블록이 사용하는 곳과 사용되는 곳의 연결만 표시합니다.",
            )
            .color(MUTED)
            .small(),
        );
    } else {
        ui.label(
            egui::RichText::new(format!("선택: {selected}"))
                .color(MUTED)
                .small(),
        );
    }
}
