use super::*;

pub(super) fn field(ui: &mut egui::Ui, label: &str, text: &mut String, multiline: bool) {
    let label = ui.label(RichText::new(label).size(11.0).color(MUTED));
    let widget = if multiline {
        egui::TextEdit::multiline(text).desired_rows(2)
    } else {
        egui::TextEdit::singleline(text)
    };
    ui.add(widget.desired_width(f32::INFINITY))
        .labelled_by(label.id);
}

pub(super) fn list_editor(ui: &mut egui::Ui, label: &str, values: &mut Vec<String>) {
    let mut text = values.join("\n");
    let label = ui.label(RichText::new(label).size(11.0).color(MUTED));
    let response = ui
        .add(
            egui::TextEdit::multiline(&mut text)
                .desired_rows(2)
                .desired_width(f32::INFINITY),
        )
        .labelled_by(label.id);
    if response.changed() {
        *values = text.split('\n').map(str::to_owned).collect();
    }
    if response.lost_focus() {
        values.retain(|value| !value.trim().is_empty());
    }
}
