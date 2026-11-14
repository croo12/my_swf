use super::*;

pub(super) fn field(ui: &mut egui::Ui, label: &str, text: &mut String, multiline: bool) {
    ui.label(RichText::new(label).size(11.0).color(MUTED));
    let widget = if multiline {
        egui::TextEdit::multiline(text).desired_rows(2)
    } else {
        egui::TextEdit::singleline(text)
    };
    ui.add(widget.desired_width(f32::INFINITY));
}

pub(super) fn list_editor(ui: &mut egui::Ui, label: &str, values: &mut Vec<String>) {
    let mut text = values.join("\n");
    ui.label(RichText::new(label).size(11.0).color(MUTED));
    let response = ui.add(
        egui::TextEdit::multiline(&mut text)
            .desired_rows(2)
            .desired_width(f32::INFINITY),
    );
    if response.changed() {
        *values = text.split('\n').map(str::to_owned).collect();
    }
    if response.lost_focus() {
        values.retain(|value| !value.trim().is_empty());
    }
}
