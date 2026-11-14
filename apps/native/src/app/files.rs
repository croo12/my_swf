use super::*;
use crate::transport;
use factory_core::repository;
impl Workbench {
    pub(super) fn open_repository(&mut self, ctx: egui::Context, path: Option<PathBuf>) {
        if self.busy {
            return;
        }
        self.busy = true;
        self.notify("저장소의 디렉터리와 import를 읽는 중…", false);
        let sender = self.sender.clone();
        std::thread::spawn(move || {
            let path = path.or_else(|| {
                rfd::FileDialog::new()
                    .set_title("FSD 저장소 열기")
                    .pick_folder()
            });
            if let Some(path) = path {
                let result = repository::analyze(&path);
                let _ = sender.send(Event::Repository(result, false));
            } else {
                let _ = sender.send(Event::Export(Ok(None)));
            }
            ctx.request_repaint();
        });
    }

    pub(super) fn remote_repository(&mut self, ctx: egui::Context) {
        if self.busy {
            return;
        }
        self.busy = true;
        self.notify("서버의 현재 저장소 구조를 읽는 중…", false);
        let (server, token, remember) = (
            self.session.server.clone(),
            self.token.clone(),
            self.remember_token,
        );
        let sender = self.sender.clone();
        std::thread::spawn(move || {
            let result = transport::repository(&server, &token).and_then(|snapshot| {
                if remember {
                    transport::token(false, &token)?;
                }
                Ok(snapshot)
            });
            let _ = sender.send(Event::Repository(result, true));
            ctx.request_repaint();
        });
    }

    pub(super) fn import(&mut self, ctx: egui::Context) {
        if self.busy {
            return;
        }
        self.busy = true;
        let sender = self.sender.clone();
        std::thread::spawn(move || {
            let result = rfd::FileDialog::new()
                .add_filter("계획 JSON", &["json"])
                .pick_file()
                .map(|file| {
                    std::fs::read_to_string(file)
                        .map_err(|e| e.to_string())
                        .and_then(|text| Plan::from_json(&text))
                })
                .transpose();
            let _ = sender.send(Event::Import(result));
            ctx.request_repaint();
        });
    }

    pub(super) fn export(&mut self, ctx: egui::Context, markdown: bool) {
        if self.busy {
            return;
        }
        let text = match self.session.plan.json() {
            Ok(json) => {
                if markdown {
                    self.session.plan.markdown()
                } else {
                    json
                }
            }
            Err(error) => {
                self.notify(error, true);
                return;
            }
        };
        self.busy = true;
        let sender = self.sender.clone();
        std::thread::spawn(move || {
            let (extension, name) = if markdown {
                ("md", "implementation-plan.md")
            } else {
                ("json", "plan.json")
            };
            let result = rfd::FileDialog::new()
                .set_file_name(name)
                .add_filter("계획", &[extension])
                .save_file()
                .map(|file| {
                    std::fs::write(&file, text)
                        .map(|_| file)
                        .map_err(|e| e.to_string())
                })
                .transpose();
            let _ = sender.send(Event::Export(result));
            ctx.request_repaint();
        });
    }

    pub(super) fn receive(&mut self) {
        while let Ok(event) = self.receiver.try_recv() {
            self.busy = false;
            match event {
                Event::Repository(Ok(snapshot), remote) => {
                    if !remote {
                        self.session.repository = snapshot.repository.clone();
                    }
                    self.notify(
                        format!(
                            "{} · 파일 {}개 · 추적 못 한 import {}건",
                            snapshot.source_root,
                            snapshot.tree.file_count,
                            snapshot.scan["analysis"]["unresolvedImports"]
                        ),
                        false,
                    );
                    self.repository = Some(Arc::new(snapshot));
                    self.remote = remote;
                    self.repository_graph.reset();
                    self.inspected_path.clear();
                    self.search.clear();
                    self.planning = false;
                    self.connection_open = false;
                    self.dirty = true;
                }
                Event::Repository(Err(error), _)
                | Event::Import(Err(error))
                | Event::Export(Err(error)) => self.notify(error, true),
                Event::Import(Ok(Some(plan))) => {
                    self.history.push(self.session.plan.clone());
                    self.redo.clear();
                    self.session.plan = plan;
                    self.selected = None;
                    self.planning = true;
                    self.dirty = true;
                    self.notify(
                        "계획을 불러왔습니다. 되돌리기로 이전 계획을 복구할 수 있습니다.",
                        false,
                    );
                }
                Event::Export(Ok(Some(file))) => {
                    self.notify(format!("내보냈습니다: {}", file.display()), false)
                }
                _ => self.notify("취소했습니다.", false),
            }
        }
    }
}
