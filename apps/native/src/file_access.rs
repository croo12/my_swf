//! File locations and native dialogs are supplied at the desktop boundary.
use crate::storage::{self, Session};
use std::{path::PathBuf, sync::Arc};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Selection {
    Repository,
    Import,
    Json,
    Markdown,
}

pub trait FileDialogs: Send + Sync {
    fn select(&self, selection: Selection) -> Option<PathBuf>;
}

struct NativeDialogs;
impl FileDialogs for NativeDialogs {
    fn select(&self, selection: Selection) -> Option<PathBuf> {
        match selection {
            Selection::Repository => rfd::FileDialog::new()
                .set_title("FSD 저장소 열기")
                .pick_folder(),
            Selection::Import => rfd::FileDialog::new()
                .add_filter("계획 JSON", &["json"])
                .pick_file(),
            Selection::Json | Selection::Markdown => {
                let (extension, name) = if selection == Selection::Markdown {
                    ("md", "implementation-plan.md")
                } else {
                    ("json", "plan.json")
                };
                rfd::FileDialog::new()
                    .set_file_name(name)
                    .add_filter("계획", &[extension])
                    .save_file()
            }
        }
    }
}

pub struct FileAccess {
    pub session_path: Result<PathBuf, String>,
    pub dialogs: Arc<dyn FileDialogs>,
}
impl FileAccess {
    pub fn native() -> Self {
        Self {
            session_path: storage::default_file(),
            dialogs: Arc::new(NativeDialogs),
        }
    }
    pub fn load(&self) -> Result<Session, String> {
        storage::load(self.session_path.as_ref().map_err(Clone::clone)?)
    }
    pub fn save(&self, session: &Session) -> Result<(), String> {
        storage::save(self.session_path.as_ref().map_err(Clone::clone)?, session)
    }
}
