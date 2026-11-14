use super::super::*;
use crate::file_access::{FileAccess, FileDialogs, Selection};
use egui_kittest::{Harness, kittest::Queryable as _};
use factory_session as session;
use std::{collections::VecDeque, path::Path, sync::Mutex};

#[derive(Default)]
pub(super) struct ScriptedDialogs {
    choices: Mutex<VecDeque<(Selection, Option<PathBuf>)>>,
}
impl ScriptedDialogs {
    pub fn push(&self, selection: Selection, path: Option<PathBuf>) {
        self.choices.lock().unwrap().push_back((selection, path));
    }
    pub fn exhausted(&self) -> bool {
        self.choices.lock().unwrap().is_empty()
    }
}
impl FileDialogs for ScriptedDialogs {
    fn select(&self, selection: Selection) -> Option<PathBuf> {
        let (expected, path) = self
            .choices
            .lock()
            .unwrap()
            .pop_front()
            .expect("An unplanned file dialog was requested");
        assert_eq!(expected, selection);
        path
    }
}

pub(super) struct Driver {
    pub h: Harness<'static, Workbench>,
    pub dialogs: Arc<ScriptedDialogs>,
    pub session_file: PathBuf,
    pub root: tempfile::TempDir,
}
impl Driver {
    pub fn new(plan: Plan) -> Self {
        let root = tempfile::tempdir().unwrap();
        session::save(
            &root.path().join("profile/session.json"),
            &Session {
                plan,
                ..Default::default()
            },
        )
        .unwrap();
        Self::from_root(root)
    }

    pub fn from_root(root: tempfile::TempDir) -> Self {
        let session_file = root.path().join("profile/session.json");
        let dialogs = Arc::new(ScriptedDialogs::default());
        let h = Self::harness(&session_file, dialogs.clone());
        let mut driver = Self {
            h,
            dialogs,
            session_file,
            root,
        };
        driver.pump();
        driver
    }

    fn harness(path: &Path, dialogs: Arc<ScriptedDialogs>) -> Harness<'static, Workbench> {
        let files = FileAccess {
            session_path: Ok(path.into()),
            dialogs,
        };
        Harness::builder()
            .with_size(egui::vec2(1440.0, 1200.0))
            .with_context_setup(|ctx| {
                // Match eframe's native system-font fallback, including Korean UI labels.
                ctx.add_font_provider(Arc::new(
                    egui_system_font_provider::SystemFontProvider::new(),
                ));
            })
            .build_eframe(move |cc| Workbench::with_file_access(cc, None, files))
    }

    pub fn restart(&mut self) {
        self.h = Self::harness(&self.session_file, self.dialogs.clone());
        self.pump();
    }

    pub fn click(&mut self, label: &str) {
        self.h.get_by_label(label).click();
        self.pump();
    }

    pub fn replace(&mut self, label: &str, text: &str) {
        self.h.get_by_label(label).focus();
        self.h.step();
        self.h
            .key_press_modifiers(egui::Modifiers::COMMAND, egui::Key::A);
        self.h.get_by_label(label).type_text(text);
        self.pump();
    }

    pub fn pump(&mut self) {
        let deadline = Instant::now() + Duration::from_secs(10);
        let mut stable_frames = 0;
        loop {
            self.h.step();
            let app = self.h.state();
            let ready = !app.busy
                && (app.planning
                    || app.repository.is_none()
                    || app
                        .repository_graph
                        .test_scene(&app.inspected_path)
                        .expect("Real layout worker failed")
                        .is_some());
            stable_frames = if ready { stable_frames + 1 } else { 0 };
            if stable_frames == 3 {
                break;
            }
            assert!(
                Instant::now() < deadline,
                "App did not finish: {}",
                app.message
            );
            std::thread::sleep(Duration::from_millis(1));
        }
        self.h.check_accessibility();
    }

    pub fn scene(&self) -> &factory_layout::Scene {
        self.h
            .state()
            .repository_graph
            .test_scene(&self.h.state().inspected_path)
            .unwrap()
            .unwrap()
    }
}
