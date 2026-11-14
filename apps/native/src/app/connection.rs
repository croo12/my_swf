use super::fields::field;
use super::*;
use crate::transport;
impl Workbench {
    pub(super) fn connection(&mut self, ctx: &egui::Context) {
        if !self.connection_open {
            return;
        }
        let mut open = true;
        egui::Window::new("원격 저장소")
            .open(&mut open)
            .collapsible(false)
            .resizable(false)
            .default_width(450.0)
            .show(ctx, |ui| {
                ui.label("현재 서버 체크아웃의 폴더와 import를 읽습니다.");
                field(ui, "서버 주소 · HTTPS", &mut self.session.server, false);
                let token_label = ui.label(RichText::new("Factory API 토큰").small().color(MUTED));
                ui.add(
                    egui::TextEdit::singleline(&mut self.token)
                        .password(true)
                        .desired_width(f32::INFINITY),
                )
                .labelled_by(token_label.id);
                ui.checkbox(
                    &mut self.remember_token,
                    "연결 성공 후 OS 자격 증명 저장소에 토큰 기억",
                );
                if ui.button("기존 앱에서 저장한 토큰 읽기").clicked() {
                    match transport::token(true, "") {
                        Ok(token) => self.token = token,
                        Err(error) => self.notify(error, true),
                    }
                }
                ui.add_space(10.0);
                if ui
                    .add_enabled(!self.busy, egui::Button::new("저장소 구조 불러오기"))
                    .clicked()
                {
                    self.remote_repository(ctx.clone());
                }
                if self.error {
                    ui.colored_label(Color32::LIGHT_RED, &self.message);
                }
            });
        if !open {
            self.connection_open = false;
        }
    }
}
